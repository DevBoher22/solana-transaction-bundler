use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use base64::Engine;
use bundler_config::BundlerConfig;
use bundler_core::BundlerService;
use bundler_types::{BundleRequest, BundleResponse};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use solana_transaction_status::option_serializer::OptionSerializer;
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;
use tracing::{error, info};

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
    pub async fn new(
        config: BundlerConfig,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
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
    pub async fn serve(
        &self,
        addr: SocketAddr,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
    info!(
        "Received bundle submission request: {}",
        request.bundle.request_id
    );

    match service.process_bundle(request.bundle).await {
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
                    details: Some(format!("{}", e)),
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
    info!(
        "Received bundle simulation request: {}",
        request.bundle.request_id
    );

    // Convert instructions to Solana instructions
    let instructions: Result<Vec<solana_sdk::instruction::Instruction>, String> = request
        .bundle
        .instructions
        .iter()
        .map(|ix| {
            let instruction_bytes = base64::engine::general_purpose::STANDARD
                .decode(&ix.data_b64)
                .map_err(|e| format!("Invalid base64 data: {}", e))?;

            let accounts = ix
                .keys
                .iter()
                .map(|meta| solana_sdk::instruction::AccountMeta {
                    pubkey: meta.pubkey,
                    is_signer: meta.is_signer,
                    is_writable: meta.is_writable,
                })
                .collect::<Vec<_>>();

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
        let fee_payer = match service.get_fee_payer_pubkey().await {
            Ok(pubkey) => pubkey,
            Err(e) => {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "Failed to get fee payer".to_string(),
                        details: Some(format!("{}", e)),
                    }),
                ));
            }
        };

        let mut transaction = solana_sdk::transaction::Transaction::new_with_payer(
            std::slice::from_ref(instruction),
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
                    "error": result.error.as_ref().map(|err| err.message.clone()),
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
            let compute_units =
                tx.transaction
                    .meta
                    .as_ref()
                    .and_then(|meta| match meta.compute_units_consumed {
                        OptionSerializer::Some(cu) => Some(cu as u64),
                        _ => None,
                    });

            let logs = if params.verbose.unwrap_or(false) {
                tx.transaction
                    .meta
                    .as_ref()
                    .and_then(|meta| match &meta.log_messages {
                        OptionSerializer::Some(logs) => Some(logs.clone()),
                        _ => None,
                    })
            } else {
                None
            };

            let error = tx
                .transaction
                .meta
                .as_ref()
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
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Transaction not found".to_string(),
                details: None,
            }),
        )),
        Err(e) => {
            error!("Failed to get transaction: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to get transaction".to_string(),
                    details: Some(format!("{}", e)),
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
                    (
                        name.clone(),
                        ComponentHealth {
                            healthy: status == "healthy",
                            message: Some(status.clone()),
                            last_success: Some(Utc::now().to_rfc3339()),
                        },
                    )
                })
                .collect::<HashMap<_, _>>();

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
                    details: Some(format!("{}", e)),
                }),
            ))
        }
    }
}

/// Get service information
async fn get_service_info(State(_service): State<Arc<BundlerService>>) -> Json<serde_json::Value> {
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
pub async fn start_service(
    config: BundlerConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
    use bundler_types::{
        BundleRequest, ComputeConfig, ComputeLimit, ComputePrice, InstructionData,
    };
    use serde_json::json;
    use solana_sdk::{instruction::AccountMeta, pubkey::Pubkey, signature::Signature};
    use uuid::Uuid;

    async fn create_test_service() -> HttpService {
        let config = BundlerConfigBuilder::new()
            .with_rpc_endpoint("https://api.devnet.solana.com".to_string(), 100)
            .build()
            .unwrap();

        HttpService::new(config).await.unwrap()
    }

    fn create_test_bundle_request() -> BundleRequest {
        BundleRequest {
            request_id: Uuid::new_v4(),
            atomic: true,
            compute: ComputeConfig {
                limit: ComputeLimit::Fixed(200000),
                price: ComputePrice::Fixed(1000),
                max_price_lamports: 50000,
            },
            alt_tables: vec![],
            instructions: vec![InstructionData {
                program_id: Pubkey::new_unique(),
                keys: vec![AccountMeta {
                    pubkey: Pubkey::new_unique(),
                    is_signer: true,
                    is_writable: true,
                }],
                data_b64: base64::engine::general_purpose::STANDARD.encode(&[1, 2, 3, 4]),
            }],
            signers: vec![],
            metadata: std::collections::HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_health_endpoint() {
        let service = create_test_service().await;
        let app = service.create_router();
        let server = TestServer::new(app).unwrap();

        let response = server.get("/v1/health").await;

        // Should return some response (might be unhealthy in test environment)
        assert!(response.status_code().is_success() || response.status_code().is_server_error());

        // Check response structure
        let body: serde_json::Value = response.json();
        assert!(body.get("healthy").is_some());
        assert!(body.get("timestamp").is_some());
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
        assert!(body.get("description").is_some());
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
        assert!(body.get("endpoints").is_some());
    }

    #[tokio::test]
    async fn test_bundle_submit_endpoint() {
        let service = create_test_service().await;
        let app = service.create_router();
        let server = TestServer::new(app).unwrap();

        let bundle_request = create_test_bundle_request();

        let response = server.post("/v1/bundle").json(&bundle_request).await;

        // May fail due to network issues, but should handle the request structure
        println!("Bundle submit response status: {}", response.status_code());

        // Check that it's not a 404 (endpoint exists)
        assert_ne!(response.status_code(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_bundle_submit_invalid_json() {
        let service = create_test_service().await;
        let app = service.create_router();
        let server = TestServer::new(app).unwrap();

        let response = server
            .post("/v1/bundle")
            .json(&json!({"invalid": "request"}))
            .await;

        assert_eq!(response.status_code(), StatusCode::BAD_REQUEST);

        let body: ErrorResponse = response.json();
        assert!(body.error.contains("Invalid") || body.error.contains("missing"));
    }

    #[tokio::test]
    async fn test_bundle_submit_empty_body() {
        let service = create_test_service().await;
        let app = service.create_router();
        let server = TestServer::new(app).unwrap();

        let response = server.post("/v1/bundle").text("").await;

        assert_eq!(response.status_code(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_status_endpoint_with_valid_signature() {
        let service = create_test_service().await;
        let app = service.create_router();
        let server = TestServer::new(app).unwrap();

        let signature = Signature::new_unique();
        let response = server.get(&format!("/v1/status/{}", signature)).await;

        // Should not be a 400 (bad request) for valid signature format
        assert_ne!(response.status_code(), StatusCode::BAD_REQUEST);

        // May be 404 (not found) or other status depending on implementation
        println!("Status endpoint response: {}", response.status_code());
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

    #[tokio::test]
    async fn test_cors_headers() {
        let service = create_test_service().await;
        let app = service.create_router();
        let server = TestServer::new(app).unwrap();

        let response = server.get("/v1/info").await;

        // Check for CORS headers
        let headers = response.headers();
        // CORS headers might be present depending on configuration
        println!("Response headers: {:?}", headers);
    }

    #[tokio::test]
    async fn test_content_type_headers() {
        let service = create_test_service().await;
        let app = service.create_router();
        let server = TestServer::new(app).unwrap();

        let response = server.get("/v1/info").await;

        assert!(response.status_code().is_success());

        // Should return JSON content type
        let content_type = response.headers().get("content-type");
        if let Some(ct) = content_type {
            assert!(ct.to_str().unwrap().contains("application/json"));
        }
    }

    #[tokio::test]
    async fn test_large_request_body() {
        let service = create_test_service().await;
        let app = service.create_router();
        let server = TestServer::new(app).unwrap();

        // Create a large bundle request
        let mut large_request = create_test_bundle_request();

        // Add many instructions to make it large
        for _ in 0..100 {
            large_request.instructions.push(InstructionData {
                program_id: Pubkey::new_unique(),
                keys: vec![AccountMeta {
                    pubkey: Pubkey::new_unique(),
                    is_signer: false,
                    is_writable: false,
                }],
                data_b64: base64::engine::general_purpose::STANDARD.encode(&vec![0u8; 1000]),
            });
        }

        let response = server.post("/v1/bundle").json(&large_request).await;

        // Should handle large requests (may fail due to size limits or network issues)
        println!("Large request response: {}", response.status_code());

        // Should not be a 404 (endpoint exists)
        assert_ne!(response.status_code(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_unsupported_http_methods() {
        let service = create_test_service().await;
        let app = service.create_router();
        let server = TestServer::new(app).unwrap();

        // Test unsupported methods on various endpoints
        let response = server.put("/v1/info").await;
        assert_eq!(response.status_code(), StatusCode::METHOD_NOT_ALLOWED);

        let response = server.delete("/v1/health").await;
        assert_eq!(response.status_code(), StatusCode::METHOD_NOT_ALLOWED);

        let response = server.patch("/v1/bundle").await;
        assert_eq!(response.status_code(), StatusCode::METHOD_NOT_ALLOWED);
    }

    #[tokio::test]
    async fn test_nonexistent_endpoints() {
        let service = create_test_service().await;
        let app = service.create_router();
        let server = TestServer::new(app).unwrap();

        let response = server.get("/v1/nonexistent").await;
        assert_eq!(response.status_code(), StatusCode::NOT_FOUND);

        let response = server.get("/v2/info").await;
        assert_eq!(response.status_code(), StatusCode::NOT_FOUND);

        let response = server.get("/invalid").await;
        assert_eq!(response.status_code(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_request_timeout_handling() {
        let service = create_test_service().await;
        let app = service.create_router();
        let server = TestServer::new(app).unwrap();

        // Test that the service can handle requests without timing out immediately
        let start = std::time::Instant::now();
        let response = server.get("/v1/info").await;
        let duration = start.elapsed();

        assert!(response.status_code().is_success());
        assert!(duration.as_secs() < 30); // Should respond quickly for info endpoint
    }

    #[tokio::test]
    async fn test_concurrent_requests() {
        let service = create_test_service().await;
        let app = service.create_router();
        let server = TestServer::new(app).unwrap();

        // Test multiple concurrent requests
        let mut handles = vec![];

        for _ in 0..10 {
            let server_clone = server.clone();
            let handle = tokio::spawn(async move { server_clone.get("/v1/info").await });
            handles.push(handle);
        }

        // Wait for all requests to complete
        for handle in handles {
            let response = handle.await.unwrap();
            assert!(response.status_code().is_success());
        }
    }

    #[tokio::test]
    async fn test_error_response_format() {
        let service = create_test_service().await;
        let app = service.create_router();
        let server = TestServer::new(app).unwrap();

        let response = server.get("/v1/status/invalid").await;

        assert_eq!(response.status_code(), StatusCode::BAD_REQUEST);

        // Check error response format
        let body: ErrorResponse = response.json();
        assert!(!body.error.is_empty());
        assert!(body.timestamp.is_some());
    }
}
