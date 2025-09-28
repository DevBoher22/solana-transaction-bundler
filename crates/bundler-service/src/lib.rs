use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use bundler_core::BundlerService;
use bundler_types::{BundleRequest, BundleResponse, BundlerError, BundlerResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use tracing::{error, info, warn};

/// HTTP service for the Solana transaction bundler
pub struct HttpService {
    bundler_service: Arc<BundlerService>,
    config: BundlerConfig,
}

/// Request to submit a bundle
#[derive(Debug, Deserialize)]
pub struct SubmitBundleRequest {
    #[serde(flatten)]
    pub bundle: BundleRequest,
}

/// Response for bundle submission
#[derive(Debug, Serialize)]
pub struct SubmitBundleResponse {
    #[serde(flatten)]
    pub response: BundleResponse,
}

/// Query parameters for status endpoint
#[derive(Debug, Deserialize)]
pub struct StatusQuery {
    /// Show verbose information
    pub verbose: Option<bool>,
}

/// Response for transaction status
#[derive(Debug, Serialize)]
pub struct TransactionStatusResponse {
    pub signature: String,
    pub status: String,
    pub slot: Option<u64>,
    pub fee: Option<u64>,
    pub compute_units: Option<u64>,
    pub logs: Option<Vec<String>>,
    pub error: Option<String>,
}

/// Response for health check
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub healthy: bool,
    pub timestamp: String,
    pub components: HashMap<String, ComponentHealth>,
}

/// Component health information
#[derive(Debug, Serialize)]
pub struct ComponentHealth {
    pub healthy: bool,
    pub message: Option<String>,
    pub last_success: Option<String>,
}

/// Error response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub details: Option<String>,
}

impl HttpService {
    /// Create a new HTTP service
    pub async fn new(config: BundlerConfig) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let bundler_service = Arc::new(BundlerService::new(config.clone()).await?);
        
        Ok(Self {
            bundler_service,
            config,
        })
    }
    
    /// Create the router with all endpoints
    pub fn create_router(&self) -> Router {
        let app_state = Arc::clone(&self.bundler_service);
        
        Router::new()
            // Bundle endpoints
            .route("/v1/bundle", post(submit_bundle))
            .route("/v1/bundle/simulate", post(simulate_bundle))
            
            // Status endpoints
            .route("/v1/status/:signature", get(get_transaction_status))
            .route("/v1/health", get(health_check))
            
            // Info endpoints
            .route("/v1/info", get(get_service_info))
            .route("/", get(root_handler))
            
            .with_state(app_state)
    }
    
    /// Start the HTTP server
    pub async fn serve(&self, addr: SocketAddr) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let app = self.create_router();
        
        info!("Starting HTTP server on {}", addr);
        
        let listener = TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;
        
        Ok(())
    }
}

/// Submit a bundle of transactions
async fn submit_bundle(
    State(service): State<Arc<BundlerService>>,
    Json(request): Json<SubmitBundleRequest>,
) -> Result<Json<SubmitBundleResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("Received bundle submission request: {}", request.bundle.request_id);
    
    match service.bundler.process_bundle(request.bundle).await {
        Ok(response) => {
            info!("Bundle processed successfully: {}", response.request_id);
            Ok(Json(SubmitBundleResponse { response }))
        }
        Err(e) => {
            error!("Bundle processing failed: {}", e);
            Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Bundle processing failed".to_string(),
                    details: Some(e.to_string()),
                }),
            ))
        }
    }
}

/// Simulate a bundle without submitting
async fn simulate_bundle(
    State(service): State<Arc<BundlerService>>,
    Json(request): Json<SubmitBundleRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    info!("Received bundle simulation request: {}", request.bundle.request_id);
    
    // Convert instructions to Solana instructions
    let instructions: Result<Vec<_>, _> = request.bundle.instructions
        .iter()
        .map(|ix| {
            let instruction_bytes = base64::engine::general_purpose::STANDARD
                .decode(&ix.data_b64)
                .map_err(|e| format!("Invalid base64 data: {}", e))?;
            
            let accounts = ix.keys
                .iter()
                .map(|meta| solana_sdk::instruction::AccountMeta {
                    pubkey: meta.pubkey,
                    is_signer: meta.is_signer,
                    is_writable: meta.is_writable,
                })
                .collect();
            
            Ok(solana_sdk::instruction::Instruction {
                program_id: ix.program_id,
                accounts,
                data: instruction_bytes,
            })
        })
        .collect();
    
    let instructions = match instructions {
        Ok(ix) => ix,
        Err(e) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Invalid instruction data".to_string(),
                    details: Some(e),
                }),
            ));
        }
    };
    
    // Simulate each instruction
    let mut simulation_results = Vec::new();
    
    for (i, instruction) in instructions.iter().enumerate() {
        // Create a simple transaction for simulation
        let fee_payer = match service.signing_manager.fee_payer_pubkey().await {
            Ok(pubkey) => pubkey,
            Err(e) => {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "Failed to get fee payer".to_string(),
                        details: Some(e.to_string()),
                    }),
                ));
            }
        };
        
        let mut transaction = solana_sdk::transaction::Transaction::new_with_payer(
            &[instruction.clone()],
            Some(&fee_payer),
        );
        
        // Set a dummy blockhash for simulation
        transaction.message.recent_blockhash = solana_sdk::hash::Hash::new_unique();
        
        match service.simulate_transaction(&transaction).await {
            Ok(result) => {
                simulation_results.push(serde_json::json!({
                    "instruction_index": i,
                    "success": result.success,
                    "compute_units_consumed": result.compute_units_consumed,
                    "estimated_fee": result.estimated_fee,
                    "logs": result.logs,
                    "error": result.error,
                }));
            }
            Err(e) => {
                simulation_results.push(serde_json::json!({
                    "instruction_index": i,
                    "success": false,
                    "error": e.to_string(),
                }));
            }
        }
    }
    
    Ok(Json(serde_json::json!({
        "request_id": request.bundle.request_id,
        "simulations": simulation_results,
    })))
}

/// Get transaction status by signature
async fn get_transaction_status(
    State(service): State<Arc<BundlerService>>,
    Path(signature_str): Path<String>,
    Query(params): Query<StatusQuery>,
) -> Result<Json<TransactionStatusResponse>, (StatusCode, Json<ErrorResponse>)> {
    let signature = match signature_str.parse::<solana_sdk::signature::Signature>() {
        Ok(sig) => sig,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Invalid signature format".to_string(),
                    details: None,
                }),
            ));
        }
    };
    
    match service.get_transaction(&signature).await {
        Ok(Some(tx)) => {
            let status = if let Some(meta) = &tx.transaction.meta {
                if meta.err.is_none() {
                    "finalized".to_string()
                } else {
                    "failed".to_string()
                }
            } else {
                "unknown".to_string()
            };
            
            let fee = tx.transaction.meta.as_ref().map(|meta| meta.fee);
            let compute_units = tx.transaction.meta.as_ref()
                .and_then(|meta| meta.compute_units_consumed.0)
                .map(|cu| cu as u64);
            
            let logs = if params.verbose.unwrap_or(false) {
                tx.transaction.meta.as_ref()
                    .and_then(|meta| meta.log_messages.0.clone())
            } else {
                None
            };
            
            let error = tx.transaction.meta.as_ref()
                .and_then(|meta| meta.err.as_ref())
                .map(|err| format!("{:?}", err));
            
            Ok(Json(TransactionStatusResponse {
                signature: signature_str,
                status,
                slot: Some(tx.slot),
                fee,
                compute_units,
                logs,
                error,
            }))
        }
        Ok(None) => {
            Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Transaction not found".to_string(),
                    details: None,
                }),
            ))
        }
        Err(e) => {
            error!("Failed to get transaction: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to get transaction".to_string(),
                    details: Some(e.to_string()),
                }),
            ))
        }
    }
}

/// Health check endpoint
async fn health_check(
    State(service): State<Arc<BundlerService>>,
) -> Result<Json<HealthResponse>, (StatusCode, Json<ErrorResponse>)> {
    match service.health_check().await {
        Ok(health) => {
            let components = health
                .iter()
                .map(|(name, status)| {
                    (name.clone(), ComponentHealth {
                        healthy: status == "healthy",
                        message: Some(status.clone()),
                        last_success: Some(Utc::now().to_rfc3339()),
                    })
                })
                .collect();
            
            let all_healthy = health.values().all(|status| status == "healthy");
            let status_code = if all_healthy {
                StatusCode::OK
            } else {
                StatusCode::SERVICE_UNAVAILABLE
            };
            
            let response = HealthResponse {
                healthy: all_healthy,
                timestamp: Utc::now().to_rfc3339(),
                components,
            };
            
            Ok(Json(response))
        }
        Err(e) => {
            error!("Health check failed: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Health check failed".to_string(),
                    details: Some(e.to_string()),
                }),
            ))
        }
    }
}

/// Get service information
async fn get_service_info(
    State(_service): State<Arc<BundlerService>>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "name": "Solana Transaction Bundler",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "Production-ready Solana transaction bundler with low latency and high success rate",
        "endpoints": {
            "submit": "POST /v1/bundle",
            "simulate": "POST /v1/bundle/simulate",
            "status": "GET /v1/status/{signature}",
            "health": "GET /v1/health",
            "info": "GET /v1/info"
        },
        "documentation": "https://github.com/your-org/solana-bundler"
    }))
}

/// Root handler
async fn root_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "message": "Solana Transaction Bundler API",
        "version": env!("CARGO_PKG_VERSION"),
        "endpoints": {
            "info": "GET /v1/info",
            "health": "GET /v1/health"
        }
    }))
}

/// Start the HTTP service
pub async fn start_service(config: BundlerConfig) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let service = HttpService::new(config.clone()).await?;
    
    let addr = SocketAddr::from(([0, 0, 0, 0], config.service.port));
    
    info!("Starting Solana Transaction Bundler HTTP service");
    info!("Listening on: http://{}", addr);
    info!("Health check: http://{}/v1/health", addr);
    info!("API info: http://{}/v1/info", addr);
    
    service.serve(addr).await?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum_test::TestServer;
    use bundler_config::BundlerConfigBuilder;
    use serde_json::json;

    async fn create_test_service() -> HttpService {
        let config = BundlerConfigBuilder::new()
            .with_rpc_endpoint("https://api.devnet.solana.com".to_string(), 100)
            .build()
            .unwrap();
        
        HttpService::new(config).await.unwrap()
    }

    #[tokio::test]
    async fn test_health_endpoint() {
        let service = create_test_service().await;
        let app = service.create_router();
        let server = TestServer::new(app).unwrap();
        
        let response = server.get("/v1/health").await;
        
        // Should return some response (might be unhealthy in test environment)
        assert!(response.status_code().is_success() || response.status_code().is_server_error());
    }

    #[tokio::test]
    async fn test_info_endpoint() {
        let service = create_test_service().await;
        let app = service.create_router();
        let server = TestServer::new(app).unwrap();
        
        let response = server.get("/v1/info").await;
        
        assert!(response.status_code().is_success());
        
        let body: serde_json::Value = response.json();
        assert!(body.get("name").is_some());
        assert!(body.get("version").is_some());
    }

    #[tokio::test]
    async fn test_root_endpoint() {
        let service = create_test_service().await;
        let app = service.create_router();
        let server = TestServer::new(app).unwrap();
        
        let response = server.get("/").await;
        
        assert!(response.status_code().is_success());
        
        let body: serde_json::Value = response.json();
        assert!(body.get("message").is_some());
    }

    #[tokio::test]
    async fn test_invalid_signature_status() {
        let service = create_test_service().await;
        let app = service.create_router();
        let server = TestServer::new(app).unwrap();
        
        let response = server.get("/v1/status/invalid-signature").await;
        
        assert_eq!(response.status_code(), StatusCode::BAD_REQUEST);
        
        let body: ErrorResponse = response.json();
        assert!(body.error.contains("Invalid signature format"));
    }
}
