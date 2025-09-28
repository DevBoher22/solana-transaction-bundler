use anyhow::{Context, Result};
use bundler_types::{BundlerError, BundlerResult, FeeStrategy, JitoConfig, RpcEndpoint, SignerConfig};
use config::{Config, Environment, File};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::{collections::HashMap, path::Path};
use tracing::{debug, info, warn};

/// Main configuration structure for the bundler
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundlerConfig {
    /// RPC configuration
    pub rpc: RpcConfig,
    
    /// Fee management configuration
    pub fees: FeeStrategy,
    
    /// Security and validation settings
    pub security: SecurityConfig,
    
    /// Signing configuration
    pub signing: SigningConfig,
    
    /// Optional Jito configuration
    pub jito: Option<JitoConfig>,
    
    /// Logging configuration
    pub logging: LoggingConfig,
    
    /// HTTP service configuration
    pub service: ServiceConfig,
    
    /// Performance tuning
    pub performance: PerformanceConfig,
}

/// RPC endpoint configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcConfig {
    /// List of RPC endpoints with weights
    pub endpoints: Vec<RpcEndpoint>,
    
    /// Default commitment level
    #[serde(default = "default_commitment")]
    pub commitment: String,
    
    /// Request timeout in seconds
    #[serde(default = "default_rpc_timeout")]
    pub timeout_seconds: u64,
    
    /// Maximum retries per request
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    
    /// Base backoff delay in milliseconds
    #[serde(default = "default_backoff_ms")]
    pub backoff_base_ms: u64,
    
    /// Maximum backoff delay in milliseconds
    #[serde(default = "default_max_backoff_ms")]
    pub backoff_max_ms: u64,
}

fn default_commitment() -> String {
    "confirmed".to_string()
}

fn default_rpc_timeout() -> u64 {
    30
}

fn default_max_retries() -> u32 {
    3
}

fn default_backoff_ms() -> u64 {
    100
}

fn default_max_backoff_ms() -> u64 {
    5000
}

/// Security and validation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Allowed program IDs (whitelist)
    pub program_whitelist: Vec<Pubkey>,
    
    /// Optional account whitelist for additional security
    pub account_whitelist: Option<Vec<Pubkey>>,
    
    /// Maximum number of writable accounts per transaction
    #[serde(default = "default_max_writable_accounts")]
    pub max_writable_accounts: usize,
    
    /// Require simulation before submission
    #[serde(default = "default_require_simulation")]
    pub require_simulation: bool,
    
    /// API key for HTTP service authentication
    pub api_key: Option<String>,
    
    /// Rate limiting configuration
    pub rate_limit: RateLimitConfig,
}

fn default_max_writable_accounts() -> usize {
    64 // Solana transaction limit
}

fn default_require_simulation() -> bool {
    true
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Requests per minute per IP
    #[serde(default = "default_requests_per_minute")]
    pub requests_per_minute: u32,
    
    /// Burst allowance
    #[serde(default = "default_burst_size")]
    pub burst_size: u32,
}

fn default_requests_per_minute() -> u32 {
    60
}

fn default_burst_size() -> u32 {
    10
}

/// Signing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningConfig {
    /// Fee payer configuration
    pub fee_payer: SignerConfig,
    
    /// Additional signers available
    #[serde(default)]
    pub additional_signers: HashMap<String, SignerConfig>,
    
    /// Signing timeout in seconds
    #[serde(default = "default_signing_timeout")]
    pub timeout_seconds: u64,
}

fn default_signing_timeout() -> u64 {
    10
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error)
    #[serde(default = "default_log_level")]
    pub level: String,
    
    /// Log format (json, pretty)
    #[serde(default = "default_log_format")]
    pub format: String,
    
    /// Log file path (optional)
    pub file: Option<String>,
    
    /// Enable request tracing
    #[serde(default = "default_enable_tracing")]
    pub enable_tracing: bool,
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_format() -> String {
    "json".to_string()
}

fn default_enable_tracing() -> bool {
    true
}

/// HTTP service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    /// Bind address
    #[serde(default = "default_bind_address")]
    pub bind_address: String,
    
    /// Port to listen on
    #[serde(default = "default_port")]
    pub port: u16,
    
    /// Enable CORS
    #[serde(default = "default_enable_cors")]
    pub enable_cors: bool,
    
    /// Request timeout in seconds
    #[serde(default = "default_service_timeout")]
    pub timeout_seconds: u64,
    
    /// Maximum request body size in bytes
    #[serde(default = "default_max_body_size")]
    pub max_body_size: usize,
}

fn default_bind_address() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    8080
}

fn default_enable_cors() -> bool {
    true
}

fn default_service_timeout() -> u64 {
    60
}

fn default_max_body_size() -> usize {
    1024 * 1024 // 1MB
}

/// Performance tuning configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Number of worker threads for async runtime
    #[serde(default = "default_worker_threads")]
    pub worker_threads: usize,
    
    /// Maximum concurrent requests
    #[serde(default = "default_max_concurrent_requests")]
    pub max_concurrent_requests: usize,
    
    /// Connection pool size per RPC endpoint
    #[serde(default = "default_connection_pool_size")]
    pub connection_pool_size: usize,
    
    /// Keep-alive timeout for HTTP connections
    #[serde(default = "default_keep_alive_timeout")]
    pub keep_alive_timeout_seconds: u64,
}

fn default_worker_threads() -> usize {
    num_cpus::get()
}

fn default_max_concurrent_requests() -> usize {
    1000
}

fn default_connection_pool_size() -> usize {
    10
}

fn default_keep_alive_timeout() -> u64 {
    30
}

impl Default for BundlerConfig {
    fn default() -> Self {
        Self {
            rpc: RpcConfig {
                endpoints: vec![
                    RpcEndpoint {
                        url: "https://api.mainnet-beta.solana.com".to_string(),
                        weight: 100,
                        supports_jito: false,
                        auth_token: None,
                    }
                ],
                commitment: default_commitment(),
                timeout_seconds: default_rpc_timeout(),
                max_retries: default_max_retries(),
                backoff_base_ms: default_backoff_ms(),
                backoff_max_ms: default_max_backoff_ms(),
            },
            fees: FeeStrategy::default(),
            security: SecurityConfig {
                program_whitelist: vec![
                    // System Program
                    "11111111111111111111111111111111".parse().unwrap(),
                    // Compute Budget Program  
                    "ComputeBudget111111111111111111111111111111".parse().unwrap(),
                ],
                account_whitelist: None,
                max_writable_accounts: default_max_writable_accounts(),
                require_simulation: default_require_simulation(),
                api_key: None,
                rate_limit: RateLimitConfig {
                    requests_per_minute: default_requests_per_minute(),
                    burst_size: default_burst_size(),
                },
            },
            signing: SigningConfig {
                fee_payer: SignerConfig {
                    signer_type: bundler_types::SignerType::Env {
                        var_name: "BUNDLER_KEYPAIR".to_string(),
                    },
                    alias: Some("fee_payer".to_string()),
                },
                additional_signers: HashMap::new(),
                timeout_seconds: default_signing_timeout(),
            },
            jito: None,
            logging: LoggingConfig {
                level: default_log_level(),
                format: default_log_format(),
                file: None,
                enable_tracing: default_enable_tracing(),
            },
            service: ServiceConfig {
                bind_address: default_bind_address(),
                port: default_port(),
                enable_cors: default_enable_cors(),
                timeout_seconds: default_service_timeout(),
                max_body_size: default_max_body_size(),
            },
            performance: PerformanceConfig {
                worker_threads: default_worker_threads(),
                max_concurrent_requests: default_max_concurrent_requests(),
                connection_pool_size: default_connection_pool_size(),
                keep_alive_timeout_seconds: default_keep_alive_timeout(),
            },
        }
    }
}

impl BundlerConfig {
    /// Load configuration from file and environment variables
    pub fn load() -> BundlerResult<Self> {
        Self::load_from_path("bundler.config.toml")
    }
    
    /// Load configuration from a specific file path
    pub fn load_from_path<P: AsRef<Path>>(path: P) -> BundlerResult<Self> {
        let mut config_builder = Config::builder()
            .add_source(Config::try_from(&BundlerConfig::default()).unwrap());
        
        // Try to load from config file if it exists
        let config_path = path.as_ref();
        if config_path.exists() {
            info!("Loading configuration from {}", config_path.display());
            config_builder = config_builder.add_source(File::from(config_path));
        } else {
            warn!("Configuration file {} not found, using defaults", config_path.display());
        }
        
        // Override with environment variables
        config_builder = config_builder.add_source(
            Environment::with_prefix("BUNDLER")
                .separator("_")
                .try_parsing(true)
        );
        
        let config = config_builder
            .build()
            .map_err(|e| BundlerError::Config(format!("Failed to build configuration: {}", e)))?;
        
        let bundler_config: BundlerConfig = config
            .try_deserialize()
            .map_err(|e| BundlerError::Config(format!("Failed to deserialize configuration: {}", e)))?;
        
        // Validate configuration
        bundler_config.validate()?;
        
        debug!("Configuration loaded successfully");
        Ok(bundler_config)
    }
    
    /// Validate the configuration
    pub fn validate(&self) -> BundlerResult<()> {
        // Validate RPC endpoints
        if self.rpc.endpoints.is_empty() {
            return Err(BundlerError::Config("At least one RPC endpoint must be configured".to_string()));
        }
        
        for endpoint in &self.rpc.endpoints {
            if endpoint.url.is_empty() {
                return Err(BundlerError::Config("RPC endpoint URL cannot be empty".to_string()));
            }
            if endpoint.weight == 0 {
                return Err(BundlerError::Config("RPC endpoint weight must be greater than 0".to_string()));
            }
        }
        
        // Validate commitment level
        match self.rpc.commitment.as_str() {
            "processed" | "confirmed" | "finalized" => {},
            _ => return Err(BundlerError::Config(
                format!("Invalid commitment level: {}", self.rpc.commitment)
            )),
        }
        
        // Validate program whitelist
        if self.security.program_whitelist.is_empty() {
            return Err(BundlerError::Config("Program whitelist cannot be empty".to_string()));
        }
        
        // Validate log level
        match self.logging.level.as_str() {
            "trace" | "debug" | "info" | "warn" | "error" => {},
            _ => return Err(BundlerError::Config(
                format!("Invalid log level: {}", self.logging.level)
            )),
        }
        
        // Validate log format
        match self.logging.format.as_str() {
            "json" | "pretty" => {},
            _ => return Err(BundlerError::Config(
                format!("Invalid log format: {}", self.logging.format)
            )),
        }
        
        // Validate performance settings
        if self.performance.worker_threads == 0 {
            return Err(BundlerError::Config("Worker threads must be greater than 0".to_string()));
        }
        
        if self.performance.max_concurrent_requests == 0 {
            return Err(BundlerError::Config("Max concurrent requests must be greater than 0".to_string()));
        }
        
        Ok(())
    }
    
    /// Save configuration to file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let toml_string = toml::to_string_pretty(self)
            .context("Failed to serialize configuration to TOML")?;
        
        std::fs::write(path, toml_string)
            .context("Failed to write configuration file")?;
        
        Ok(())
    }
    
    /// Get the primary RPC endpoint
    pub fn primary_rpc_endpoint(&self) -> &RpcEndpoint {
        self.rpc.endpoints
            .iter()
            .max_by_key(|endpoint| endpoint.weight)
            .expect("At least one RPC endpoint must be configured")
    }
    
    /// Get RPC endpoints sorted by weight (descending)
    pub fn rpc_endpoints_by_weight(&self) -> Vec<&RpcEndpoint> {
        let mut endpoints = self.rpc.endpoints.iter().collect::<Vec<_>>();
        endpoints.sort_by(|a, b| b.weight.cmp(&a.weight));
        endpoints
    }
    
    /// Check if a program ID is whitelisted
    pub fn is_program_whitelisted(&self, program_id: &Pubkey) -> bool {
        self.security.program_whitelist.contains(program_id)
    }
    
    /// Check if an account is whitelisted (if whitelist is enabled)
    pub fn is_account_whitelisted(&self, account: &Pubkey) -> bool {
        match &self.security.account_whitelist {
            Some(whitelist) => whitelist.contains(account),
            None => true, // No whitelist means all accounts are allowed
        }
    }
}

impl SecurityConfig {
    /// Check if a program ID is whitelisted
    pub fn is_program_whitelisted(&self, program_id: &Pubkey) -> bool {
        self.program_whitelist.contains(program_id)
    }
    
    /// Check if an account is whitelisted (if whitelist is enabled)
    pub fn is_account_whitelisted(&self, account: &Pubkey) -> bool {
        match &self.account_whitelist {
            Some(whitelist) => whitelist.contains(account),
            None => true, // No whitelist means all accounts are allowed
        }
    }
}

/// Configuration builder for easier programmatic configuration
pub struct BundlerConfigBuilder {
    config: BundlerConfig,
}

impl BundlerConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: BundlerConfig::default(),
        }
    }
    
    pub fn with_rpc_endpoint(mut self, url: String, weight: u32) -> Self {
        self.config.rpc.endpoints.push(RpcEndpoint {
            url,
            weight,
            supports_jito: false,
            auth_token: None,
        });
        self
    }
    
    pub fn with_jito_config(mut self, jito_config: JitoConfig) -> Self {
        self.config.jito = Some(jito_config);
        self
    }
    
    pub fn with_program_whitelist(mut self, programs: Vec<Pubkey>) -> Self {
        self.config.security.program_whitelist = programs;
        self
    }
    
    pub fn with_log_level(mut self, level: String) -> Self {
        self.config.logging.level = level;
        self
    }
    
    pub fn build(self) -> BundlerResult<BundlerConfig> {
        self.config.validate()?;
        Ok(self.config)
    }
}

impl Default for BundlerConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// Add num_cpus dependency to Cargo.toml for the default worker threads calculation
extern crate num_cpus;

// Re-export commonly used program IDs
pub mod program_ids {
    use solana_sdk::pubkey::Pubkey;
    
    pub fn system_program() -> Pubkey {
        "11111111111111111111111111111111".parse().unwrap()
    }
    
    pub fn compute_budget() -> Pubkey {
        "ComputeBudget111111111111111111111111111111".parse().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    
    #[test]
    fn test_default_config_validation() {
        let config = BundlerConfig::default();
        assert!(config.validate().is_ok());
    }
    
    #[test]
    fn test_config_builder() {
        let config = BundlerConfigBuilder::new()
            .with_rpc_endpoint("https://api.devnet.solana.com".to_string(), 100)
            .with_log_level("debug".to_string())
            .build()
            .unwrap();
        
        assert_eq!(config.logging.level, "debug");
        assert!(config.rpc.endpoints.len() >= 1);
    }
    
    #[test]
    fn test_config_serialization() {
        let config = BundlerConfig::default();
        let toml_string = toml::to_string(&config).unwrap();
        let deserialized: BundlerConfig = toml::from_str(&toml_string).unwrap();
        
        assert_eq!(config.rpc.commitment, deserialized.rpc.commitment);
        assert_eq!(config.logging.level, deserialized.logging.level);
    }
    
    #[test]
    fn test_config_file_operations() {
        let config = BundlerConfig::default();
        let temp_file = NamedTempFile::new().unwrap();
        
        // Save config
        config.save_to_file(temp_file.path()).unwrap();
        
        // Load config
        let loaded_config = BundlerConfig::load_from_path(temp_file.path()).unwrap();
        
        assert_eq!(config.rpc.commitment, loaded_config.rpc.commitment);
        assert_eq!(config.logging.level, loaded_config.logging.level);
    }
    
    #[test]
    fn test_program_whitelist_check() {
        let config = BundlerConfig::default();
        
        // System program should be whitelisted by default
        assert!(config.is_program_whitelisted(&solana_sdk::system_program::id()));
        
        // Random program should not be whitelisted
        let random_program = Pubkey::new_unique();
        assert!(!config.is_program_whitelisted(&random_program));
    }
}
