//! Core bundling logic and RPC client for the Solana transaction bundler
//! 
//! This crate provides the fundamental building blocks for bundling Solana transactions:
//! - RPC client with intelligent failover and health monitoring
//! - Adaptive fee management system with trend analysis
//! - Transaction bundling and simulation logic
//! - Signing infrastructure with KMS support

pub mod rpc;
pub mod fees;
pub mod bundler;
pub mod signing;
pub mod simulation;

// Re-export commonly used types and functions
pub use bundler::{TransactionBundler, BundleResult};
pub use fees::{FeeManager, FeeStatistics, FeePriority, FeeContext};
pub use rpc::{SolanaRpcClient, EndpointHealth};
pub use signing::{SigningManager, KeyProvider};
pub use simulation::{TransactionSimulator, SimulationResult};

use bundler_config::BundlerConfig;
use bundler_types::BundlerResult;
use std::sync::Arc;

/// Main bundler service that orchestrates all components
pub struct BundlerService {
    pub rpc_client: Arc<SolanaRpcClient>,
    pub fee_manager: Arc<FeeManager>,
    pub signing_manager: Arc<SigningManager>,
    pub simulator: Arc<TransactionSimulator>,
    pub bundler: Arc<TransactionBundler>,
    pub config: BundlerConfig,
}

impl BundlerService {
    /// Create a new bundler service with the given configuration
    pub async fn new(config: BundlerConfig) -> BundlerResult<Self> {
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

        Ok(Self {
            rpc_client,
            fee_manager,
            signing_manager,
            simulator,
            bundler,
            config,
        })
    }
    
    /// Perform health check on all components
    pub async fn health_check(&self) -> BundlerResult<bundler_types::HealthStatus> {
        use bundler_types::{HealthStatus, ComponentHealth};
        use chrono::Utc;
        use std::collections::HashMap;
        
        let mut components = HashMap::new();
        let mut overall_healthy = true;
        
        // Check RPC client health
        match self.rpc_client.health_check().await {
            Ok(_) => {
                components.insert("rpc_client".to_string(), ComponentHealth {
                    healthy: true,
                    message: Some("All RPC endpoints responding".to_string()),
                    last_success: Some(Utc::now()),
                });
            }
            Err(e) => {
                overall_healthy = false;
                components.insert("rpc_client".to_string(), ComponentHealth {
                    healthy: false,
                    message: Some(format!("RPC health check failed: {}", e)),
                    last_success: None,
                });
            }
        }
        
        // Check signing manager
        match self.signing_manager.health_check().await {
            Ok(_) => {
                components.insert("signing_manager".to_string(), ComponentHealth {
                    healthy: true,
                    message: Some("All signers accessible".to_string()),
                    last_success: Some(Utc::now()),
                });
            }
            Err(e) => {
                overall_healthy = false;
                components.insert("signing_manager".to_string(), ComponentHealth {
                    healthy: false,
                    message: Some(format!("Signing health check failed: {}", e)),
                    last_success: None,
                });
            }
        }
        
        // Add fee manager status
        let fee_stats = self.fee_manager.get_fee_statistics();
        components.insert("fee_manager".to_string(), ComponentHealth {
            healthy: true,
            message: Some(format!("Tracking {} fee samples", fee_stats.sample_count)),
            last_success: fee_stats.last_updated,
        });
        
        Ok(HealthStatus {
            healthy: overall_healthy,
            components,
            timestamp: Utc::now(),
        })
    }
    
    /// Get service metrics for monitoring
    pub async fn get_metrics(&self) -> BundlerResult<ServiceMetrics> {
        let rpc_health = self.rpc_client.get_health_status();
        let fee_stats = self.fee_manager.get_fee_statistics();
        
        Ok(ServiceMetrics {
            rpc_endpoints: rpc_health,
            fee_statistics: fee_stats,
            uptime_seconds: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        })
    }
}

/// Service metrics for monitoring and observability
#[derive(Debug, Clone)]
pub struct ServiceMetrics {
    pub rpc_endpoints: std::collections::HashMap<String, EndpointHealth>,
    pub fee_statistics: FeeStatistics,
    pub uptime_seconds: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use bundler_config::BundlerConfigBuilder;

    #[tokio::test]
    async fn test_bundler_service_creation() {
        let config = BundlerConfigBuilder::new()
            .with_rpc_endpoint("https://api.devnet.solana.com".to_string(), 100)
            .build()
            .unwrap();
        
        let service = BundlerService::new(config).await;
        assert!(service.is_ok());
    }
}
