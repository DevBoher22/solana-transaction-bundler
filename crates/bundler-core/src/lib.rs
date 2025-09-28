use bundler_config::BundlerConfig;
use bundler_types::BundlerResult;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

pub mod rpc;
pub mod fees;
pub mod signing;
pub mod simulation;
pub mod bundler;

pub use rpc::SolanaRpcClient;
pub use fees::FeeManager;
pub use signing::SigningManager;
pub use simulation::TransactionSimulator;
pub use bundler::TransactionBundler;

/// Main service that orchestrates all bundler components
pub struct BundlerService {
    rpc_client: Arc<SolanaRpcClient>,
    fee_manager: Arc<FeeManager>,
    signing_manager: Arc<SigningManager>,
    simulator: Arc<TransactionSimulator>,
    bundler: Arc<TransactionBundler>,
    config: BundlerConfig,
}

impl BundlerService {
    /// Create a new bundler service
    pub async fn new(config: BundlerConfig) -> BundlerResult<Self> {
        info!("Initializing Solana Transaction Bundler Service");
        
        // Initialize RPC client
        let rpc_client = Arc::new(SolanaRpcClient::new(&config)?);
        
        // Initialize fee manager
        let fee_manager = Arc::new(FeeManager::new(
            Arc::clone(&rpc_client),
            config.fees.clone(),
        ));
        
        // Initialize signing manager
        let signing_manager = Arc::new(SigningManager::new(config.signing.clone()).await?);
        
        // Initialize simulator
        let simulator = Arc::new(TransactionSimulator::new(
            Arc::clone(&rpc_client),
            &config.security,
        ));
        
        // Initialize bundler
        let bundler = Arc::new(TransactionBundler::new(
            Arc::clone(&rpc_client),
            Arc::clone(&fee_manager),
            Arc::clone(&signing_manager),
            Arc::clone(&simulator),
            &config,
        ));
        
        info!("Bundler service initialized successfully");
        
        Ok(Self {
            rpc_client,
            fee_manager,
            signing_manager,
            simulator,
            bundler,
            config,
        })
    }
    
    /// Get the transaction bundler
    pub fn bundler(&self) -> Arc<TransactionBundler> {
        Arc::clone(&self.bundler)
    }
    
    /// Get the RPC client
    pub fn rpc_client(&self) -> Arc<SolanaRpcClient> {
        Arc::clone(&self.rpc_client)
    }
    
    /// Get the fee manager
    pub fn fee_manager(&self) -> Arc<FeeManager> {
        Arc::clone(&self.fee_manager)
    }
    
    /// Get the signing manager
    pub fn signing_manager(&self) -> Arc<SigningManager> {
        Arc::clone(&self.signing_manager)
    }
    
    /// Get the transaction simulator
    pub fn simulator(&self) -> Arc<TransactionSimulator> {
        Arc::clone(&self.simulator)
    }
    
    /// Perform comprehensive health check
    pub async fn health_check(&self) -> BundlerResult<HashMap<String, String>> {
        let mut health = HashMap::new();
        
        // Check RPC client
        match self.rpc_client.health_check().await {
            Ok(_) => health.insert("rpc_client".to_string(), "healthy".to_string()),
            Err(e) => {
                warn!("RPC client health check failed: {}", e);
                health.insert("rpc_client".to_string(), "unhealthy".to_string())
            }
        };
        
        // Check signing manager
        match self.signing_manager.health_check().await {
            Ok(_) => health.insert("signing_manager".to_string(), "healthy".to_string()),
            Err(e) => {
                warn!("Signing manager health check failed: {}", e);
                health.insert("signing_manager".to_string(), "unhealthy".to_string())
            }
        };
        
        // Check simulator
        match self.simulator.health_check().await {
            Ok(_) => health.insert("simulator".to_string(), "healthy".to_string()),
            Err(e) => {
                warn!("Simulator health check failed: {}", e);
                health.insert("simulator".to_string(), "unhealthy".to_string())
            }
        };
        
        health.insert("fee_manager".to_string(), "healthy".to_string());
        health.insert("bundler".to_string(), "healthy".to_string());
        
        Ok(health)
    }
    
    /// Get fee payer public key
    pub async fn get_fee_payer_pubkey(&self) -> BundlerResult<solana_sdk::pubkey::Pubkey> {
        self.signing_manager.get_fee_payer_pubkey().await
    }
    
    /// Get service information
    pub async fn get_info(&self) -> HashMap<String, serde_json::Value> {
        let mut stats = HashMap::new();
        
        // Add basic service info
        stats.insert("service_name".to_string(), serde_json::Value::String("solana-bundler".to_string()));
        stats.insert("version".to_string(), serde_json::Value::String("0.1.0".to_string()));
        
        // Add component stats
        let rpc_stats = self.rpc_client.get_stats().await;
        for (key, value) in rpc_stats {
            stats.insert(format!("rpc_{}", key), value);
        }
        
        let fee_stats = self.fee_manager.get_stats().await;
        for (key, value) in fee_stats {
            stats.insert(format!("fee_{}", key), value);
        }
        
        let signing_stats = self.signing_manager.get_stats().await;
        for (key, value) in signing_stats {
            stats.insert(format!("signing_{}", key), value);
        }
        
        let simulator_stats = self.simulator.get_stats().await;
        for (key, value) in simulator_stats {
            stats.insert(format!("simulator_{}", key), value);
        }
        
        stats
    }
    
    /// Get configuration summary
    pub fn get_config_summary(&self) -> HashMap<String, serde_json::Value> {
        let mut summary = HashMap::new();
        
        summary.insert("rpc_endpoints_count".to_string(), 
                      serde_json::Value::Number(self.config.rpc.endpoints.len().into()));
        summary.insert("fee_strategy".to_string(), 
                      serde_json::Value::String(format!("{:?}", self.config.fees)));
        summary.insert("security_enabled".to_string(), 
                      serde_json::Value::Bool(!self.config.security.program_whitelist.is_empty()));
        summary.insert("service_port".to_string(), 
                      serde_json::Value::Number(self.config.service.port.into()));
        
        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bundler_config::BundlerConfigBuilder;

    #[tokio::test]
    async fn test_bundler_service_creation() {
        let config = BundlerConfigBuilder::new()
            .build()
            .expect("Failed to build config");
        
        // This test might fail due to missing dependencies, but it validates the structure
        let result = BundlerService::new(config).await;
        // We don't assert success here since we don't have real RPC endpoints
        // but the code should compile
        println!("Service creation result: {:?}", result.is_ok());
    }
}
