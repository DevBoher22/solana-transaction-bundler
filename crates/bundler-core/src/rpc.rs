use anyhow::Result;
use backoff::ExponentialBackoffBuilder;
use bundler_config::{BundlerConfig, RpcConfig};
use bundler_types::{BundlerError, BundlerResult, RpcEndpoint};
use chrono::{DateTime, Utc};
use reqwest::{Client, ClientBuilder};
use serde_json::{json, Value};
use solana_client::{
    rpc_client::RpcClient,
    rpc_config::{RpcSendTransactionConfig, RpcSimulateTransactionConfig},
    rpc_response::RpcSimulateTransactionResult,
};
use solana_sdk::{
    commitment_config::{CommitmentConfig, CommitmentLevel},
    hash::Hash,
    pubkey::Pubkey,
    signature::Signature,
    transaction::Transaction,
};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

/// Health status of an RPC endpoint
#[derive(Debug, Clone)]
pub struct EndpointHealth {
    pub healthy: bool,
    pub last_success: Option<DateTime<Utc>>,
    pub last_failure: Option<DateTime<Utc>>,
    pub consecutive_failures: u32,
    pub average_latency_ms: Option<u64>,
}

impl Default for EndpointHealth {
    fn default() -> Self {
        Self {
            healthy: true,
            last_success: None,
            last_failure: None,
            consecutive_failures: 0,
            average_latency_ms: None,
        }
    }
}

/// RPC client with intelligent failover and health monitoring
pub struct SolanaRpcClient {
    endpoints: Vec<RpcEndpoint>,
    clients: HashMap<String, RpcClient>,
    http_client: Client,
    health_status: Arc<RwLock<HashMap<String, EndpointHealth>>>,
    config: RpcConfig,
}

impl SolanaRpcClient {
    /// Create a new RPC client with the given configuration
    pub fn new(config: &BundlerConfig) -> BundlerResult<Self> {
        let http_client = ClientBuilder::new()
            .timeout(Duration::from_secs(config.rpc.timeout_seconds))
            .pool_max_idle_per_host(config.performance.connection_pool_size)
            .pool_idle_timeout(Duration::from_secs(config.performance.keep_alive_timeout_seconds))
            .build()
            .map_err(|e| BundlerError::Rpc(format!("Failed to create HTTP client: {}", e)))?;

        let mut clients = HashMap::new();
        let mut health_status = HashMap::new();

        for endpoint in &config.rpc.endpoints {
            let rpc_client = RpcClient::new_with_timeout_and_commitment(
                endpoint.url.clone(),
                Duration::from_secs(config.rpc.timeout_seconds),
                CommitmentConfig {
                    commitment: match config.rpc.commitment.as_str() {
                        "processed" => CommitmentLevel::Processed,
                        "confirmed" => CommitmentLevel::Confirmed,
                        "finalized" => CommitmentLevel::Finalized,
                        _ => CommitmentLevel::Confirmed,
                    },
                },
            );

            clients.insert(endpoint.url.clone(), rpc_client);
            health_status.insert(endpoint.url.clone(), EndpointHealth::default());
        }

        Ok(Self {
            endpoints: config.rpc.endpoints.clone(),
            clients,
            http_client,
            health_status: Arc::new(RwLock::new(health_status)),
            config: config.rpc.clone(),
        })
    }

    /// Get the best available RPC endpoint based on health and weight
    pub fn get_best_endpoint(&self) -> BundlerResult<&RpcEndpoint> {
        let health_status = self.health_status.read().unwrap();
        
        // Filter healthy endpoints and sort by weight
        let mut healthy_endpoints: Vec<&RpcEndpoint> = self.endpoints
            .iter()
            .filter(|endpoint| {
                health_status
                    .get(&endpoint.url)
                    .map(|health| health.healthy)
                    .unwrap_or(true)
            })
            .collect();

        if healthy_endpoints.is_empty() {
            // If no healthy endpoints, use the highest weight endpoint as fallback
            warn!("No healthy RPC endpoints available, using fallback");
            return self.endpoints
                .iter()
                .max_by_key(|endpoint| endpoint.weight)
                .ok_or_else(|| BundlerError::Rpc("No RPC endpoints configured".to_string()));
        }

        // Sort by weight (descending) and return the best one
        healthy_endpoints.sort_by(|a, b| b.weight.cmp(&a.weight));
        Ok(healthy_endpoints[0])
    }

    /// Execute an RPC call with automatic failover
    pub async fn execute_with_failover<F, T>(&self, operation: F) -> BundlerResult<T>
    where
        F: Fn(&RpcClient) -> Result<T, solana_client::client_error::ClientError> + Clone,
    {
        let mut last_error = None;
        let mut attempts = 0;
        let max_attempts = self.config.max_retries + 1;

        while attempts < max_attempts {
            let endpoint = self.get_best_endpoint()?;
            let client = self.clients.get(&endpoint.url)
                .ok_or_else(|| BundlerError::Rpc(format!("Client not found for endpoint: {}", endpoint.url)))?;

            let start_time = Instant::now();
            
            match operation(client) {
                Ok(result) => {
                    let latency = start_time.elapsed().as_millis() as u64;
                    self.record_success(&endpoint.url, latency);
                    return Ok(result);
                }
                Err(e) => {
                    let latency = start_time.elapsed().as_millis() as u64;
                    self.record_failure(&endpoint.url, latency);
                    last_error = Some(e);
                    attempts += 1;

                    if attempts < max_attempts {
                        let backoff = self.calculate_backoff(attempts);
                        debug!("RPC call failed, retrying in {:?}. Attempt {}/{}", backoff, attempts, max_attempts);
                        tokio::time::sleep(backoff).await;
                    }
                }
            }
        }

        Err(BundlerError::Rpc(format!(
            "All RPC attempts failed. Last error: {:?}",
            last_error.unwrap()
        )))
    }

    /// Send a transaction with automatic failover
    pub async fn send_transaction(&self, transaction: &Transaction) -> BundlerResult<Signature> {
        let config = RpcSendTransactionConfig {
            skip_preflight: false,
            preflight_commitment: Some(CommitmentLevel::Processed),
            encoding: None,
            max_retries: Some(0), // We handle retries ourselves
            min_context_slot: None,
        };

        self.execute_with_failover(|client| {
            client.send_transaction_with_config(transaction, config)
        }).await
    }

    /// Simulate a transaction
    pub async fn simulate_transaction(&self, transaction: &Transaction) -> BundlerResult<RpcSimulateTransactionResult> {
        let config = RpcSimulateTransactionConfig {
            sig_verify: true,
            replace_recent_blockhash: true,
            commitment: Some(CommitmentConfig::processed()),
            encoding: None,
            accounts: None,
            min_context_slot: None,
            inner_instructions: true,
        };

        self.execute_with_failover(|client| {
            client.simulate_transaction_with_config(transaction, config)
        }).await
    }

    /// Get the latest blockhash
    pub async fn get_latest_blockhash(&self) -> BundlerResult<Hash> {
        self.execute_with_failover(|client| {
            client.get_latest_blockhash()
        }).await
    }

    /// Get recent prioritization fees
    pub async fn get_recent_prioritization_fees(&self, accounts: &[Pubkey]) -> BundlerResult<Vec<u64>> {
        // Use HTTP client for this call as it's not available in the standard RPC client
        let endpoint = self.get_best_endpoint()?;
        
        let request_body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getRecentPrioritizationFees",
            "params": [
                accounts.iter().map(|pk| pk.to_string()).collect::<Vec<_>>()
            ]
        });

        let response = timeout(
            Duration::from_secs(self.config.timeout_seconds),
            self.http_client
                .post(&endpoint.url)
                .json(&request_body)
                .send()
        ).await
        .map_err(|_| BundlerError::Rpc("Request timeout".to_string()))?
        .map_err(|e| BundlerError::Rpc(format!("HTTP request failed: {}", e)))?;

        let response_json: Value = response
            .json()
            .await
            .map_err(|e| BundlerError::Rpc(format!("Failed to parse response: {}", e)))?;

        if let Some(error) = response_json.get("error") {
            return Err(BundlerError::Rpc(format!("RPC error: {}", error)));
        }

        let result = response_json
            .get("result")
            .ok_or_else(|| BundlerError::Rpc("No result in response".to_string()))?;

        let fees: Vec<u64> = result
            .as_array()
            .ok_or_else(|| BundlerError::Rpc("Invalid result format".to_string()))?
            .iter()
            .filter_map(|fee_data| {
                fee_data
                    .get("prioritizationFee")
                    .and_then(|fee| fee.as_u64())
            })
            .collect();

        Ok(fees)
    }

    /// Get transaction status
    pub async fn get_transaction(&self, signature: &Signature) -> BundlerResult<Option<solana_transaction_status::EncodedConfirmedTransactionWithStatusMeta>> {
        self.execute_with_failover(|client| {
            client.get_transaction(signature, solana_client::rpc_config::RpcTransactionConfig::default())
        }).await
    }

    /// Check if a transaction is confirmed at the specified commitment level
    pub async fn confirm_transaction(&self, signature: &Signature, commitment: CommitmentLevel) -> BundlerResult<bool> {
        self.execute_with_failover(|client| {
            client.confirm_transaction_with_commitment(signature, CommitmentConfig { commitment })
                .map(|response| response.value)
        }).await
    }

    /// Get account information
    pub async fn get_account(&self, pubkey: &Pubkey) -> BundlerResult<Option<solana_sdk::account::Account>> {
        self.execute_with_failover(|client| {
            client.get_account(pubkey)
        }).await
    }

    /// Record a successful operation for health monitoring
    fn record_success(&self, endpoint_url: &str, latency_ms: u64) {
        let mut health_status = self.health_status.write().unwrap();
        if let Some(health) = health_status.get_mut(endpoint_url) {
            health.healthy = true;
            health.last_success = Some(Utc::now());
            health.consecutive_failures = 0;
            
            // Update average latency (simple moving average)
            health.average_latency_ms = Some(
                health.average_latency_ms
                    .map(|avg| (avg + latency_ms) / 2)
                    .unwrap_or(latency_ms)
            );
        }
    }

    /// Record a failed operation for health monitoring
    fn record_failure(&self, endpoint_url: &str, _latency_ms: u64) {
        let mut health_status = self.health_status.write().unwrap();
        if let Some(health) = health_status.get_mut(endpoint_url) {
            health.last_failure = Some(Utc::now());
            health.consecutive_failures += 1;
            
            // Mark as unhealthy after 3 consecutive failures
            if health.consecutive_failures >= 3 {
                health.healthy = false;
                warn!("Marking endpoint {} as unhealthy after {} consecutive failures", 
                      endpoint_url, health.consecutive_failures);
            }
        }
    }

    /// Calculate backoff delay for retries
    fn calculate_backoff(&self, attempt: u32) -> Duration {
        let backoff = ExponentialBackoffBuilder::new()
            .with_initial_interval(Duration::from_millis(self.config.backoff_base_ms))
            .with_max_interval(Duration::from_millis(self.config.backoff_max_ms))
            .with_multiplier(2.0)
            .with_max_elapsed_time(None)
            .build();

        // Calculate the delay for this attempt
        let mut current_interval = Duration::from_millis(self.config.backoff_base_ms);
        for _ in 1..attempt {
            current_interval = std::cmp::min(
                Duration::from_millis((current_interval.as_millis() as f64 * 2.0) as u64),
                Duration::from_millis(self.config.backoff_max_ms)
            );
        }

        // Add jitter (±25%)
        let jitter_range = current_interval.as_millis() as f64 * 0.25;
        let jitter = (rand::random::<f64>() - 0.5) * 2.0 * jitter_range;
        let final_delay = (current_interval.as_millis() as f64 + jitter).max(0.0) as u64;

        Duration::from_millis(final_delay)
    }

    /// Get health status of all endpoints
    pub fn get_health_status(&self) -> HashMap<String, EndpointHealth> {
        self.health_status.read().unwrap().clone()
    }

    /// Perform health check on all endpoints
    pub async fn health_check(&self) -> BundlerResult<()> {
        let mut tasks = Vec::new();

        for endpoint in &self.endpoints {
            let client = self.clients.get(&endpoint.url).unwrap().clone();
            let endpoint_url = endpoint.url.clone();
            let health_status = Arc::clone(&self.health_status);

            let task = tokio::spawn(async move {
                let start_time = Instant::now();
                match client.get_health().await {
                    Ok(_) => {
                        let latency = start_time.elapsed().as_millis() as u64;
                        let mut health_status = health_status.write().unwrap();
                        if let Some(health) = health_status.get_mut(&endpoint_url) {
                            health.healthy = true;
                            health.last_success = Some(Utc::now());
                            health.consecutive_failures = 0;
                            health.average_latency_ms = Some(
                                health.average_latency_ms
                                    .map(|avg| (avg + latency) / 2)
                                    .unwrap_or(latency)
                            );
                        }
                        debug!("Health check passed for {}", endpoint_url);
                    }
                    Err(e) => {
                        let mut health_status = health_status.write().unwrap();
                        if let Some(health) = health_status.get_mut(&endpoint_url) {
                            health.last_failure = Some(Utc::now());
                            health.consecutive_failures += 1;
                            if health.consecutive_failures >= 3 {
                                health.healthy = false;
                            }
                        }
                        warn!("Health check failed for {}: {}", endpoint_url, e);
                    }
                }
            });

            tasks.push(task);
        }

        // Wait for all health checks to complete
        for task in tasks {
            let _ = task.await;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bundler_config::BundlerConfigBuilder;
    use bundler_types::RpcEndpoint;

    fn create_test_config() -> BundlerConfig {
        BundlerConfigBuilder::new()
            .with_rpc_endpoint("https://api.devnet.solana.com".to_string(), 100)
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn test_rpc_client_creation() {
        let config = create_test_config();
        let client = SolanaRpcClient::new(&config);
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_best_endpoint_selection() {
        let config = create_test_config();
        let client = SolanaRpcClient::new(&config).unwrap();
        let endpoint = client.get_best_endpoint();
        assert!(endpoint.is_ok());
    }

    #[test]
    fn test_backoff_calculation() {
        let config = create_test_config();
        let client = SolanaRpcClient::new(&config).unwrap();
        
        let backoff1 = client.calculate_backoff(1);
        let backoff2 = client.calculate_backoff(2);
        let backoff3 = client.calculate_backoff(3);
        
        // Backoff should generally increase with attempts (allowing for jitter)
        assert!(backoff1.as_millis() > 0);
        assert!(backoff2.as_millis() > 0);
        assert!(backoff3.as_millis() > 0);
    }

    #[test]
    fn test_health_status_tracking() {
        let config = create_test_config();
        let client = SolanaRpcClient::new(&config).unwrap();
        
        let endpoint_url = "https://api.devnet.solana.com";
        
        // Record success
        client.record_success(endpoint_url, 100);
        let health = client.get_health_status();
        assert!(health.get(endpoint_url).unwrap().healthy);
        
        // Record multiple failures
        client.record_failure(endpoint_url, 1000);
        client.record_failure(endpoint_url, 1000);
        client.record_failure(endpoint_url, 1000);
        
        let health = client.get_health_status();
        assert!(!health.get(endpoint_url).unwrap().healthy);
    }
}
