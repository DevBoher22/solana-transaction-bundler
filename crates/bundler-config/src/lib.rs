use anyhow::{Context, Result};
use bundler_types::{BundlerError, BundlerResult, FeeStrategy, JitoConfig, RpcEndpoint, SignerConfig};
use config::{Config, Environment, File};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::path::Path;

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
    
    /// Maximum number of retries
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    
    /// Connection pool size
    #[serde(default = "default_pool_size")]
    pub pool_size: u32,
}

/// Security and validation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Maximum compute units per transaction
    #[serde(default = "default_max_compute_units")]
    pub max_compute_units: u32,
    
    /// Maximum fee per transaction (lamports)
    #[serde(default = "default_max_fee_lamports")]
    pub max_fee_lamports: u64,
    
    /// Program whitelist (empty = allow all)
    #[serde(default)]
    pub program_whitelist: Vec<Pubkey>,
    
    /// Whether to validate instructions
    #[serde(default = "default_validate_instructions")]
    pub validate_instructions: bool,
    
    /// Maximum bundle size
    #[serde(default = "default_max_bundle_size")]
    pub max_bundle_size: u32,

    /// Whether transactions must be simulated before submission
    #[serde(default = "default_require_simulation")]
    pub require_simulation: bool,

    /// Maximum number of writable accounts allowed per instruction
    #[serde(default = "default_max_writable_accounts")]
    pub max_writable_accounts: u32,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            max_compute_units: default_max_compute_units(),
            max_fee_lamports: default_max_fee_lamports(),
            program_whitelist: vec![system_program()],
            validate_instructions: default_validate_instructions(),
            max_bundle_size: default_max_bundle_size(),
            require_simulation: default_require_simulation(),
            max_writable_accounts: default_max_writable_accounts(),
        }
    }
}

impl SecurityConfig {
    /// Check if a program is whitelisted. Empty whitelist allows all programs.
    pub fn is_program_whitelisted(&self, program_id: &Pubkey) -> bool {
        self.program_whitelist.is_empty() || self.program_whitelist.contains(program_id)
    }
}

/// Signing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningConfig {
    /// Fee payer configuration
    pub fee_payer: SignerConfig,
    
    /// Additional signers
    #[serde(default)]
    pub additional_signers: Vec<SignerConfig>,
    
    /// Whether to use parallel signing
    #[serde(default = "default_parallel_signing")]
    pub parallel_signing: bool,
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
    
    /// Whether to log to file
    #[serde(default)]
    pub file_enabled: bool,
    
    /// Log file path
    pub file_path: Option<String>,
    
    /// Whether to include timestamps
    #[serde(default = "default_include_timestamps")]
    pub include_timestamps: bool,
}

/// HTTP service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    /// Server bind address
    #[serde(default = "default_bind_address")]
    pub bind_address: String,
    
    /// Server port
    #[serde(default = "default_port")]
    pub port: u16,
    
    /// Request timeout in seconds
    #[serde(default = "default_request_timeout")]
    pub request_timeout_seconds: u64,
    
    /// Maximum request body size
    #[serde(default = "default_max_body_size")]
    pub max_body_size_bytes: u64,
    
    /// CORS configuration
    pub cors: CorsConfig,
    
    /// Rate limiting
    pub rate_limit: RateLimitConfig,
}

/// CORS configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorsConfig {
    /// Allowed origins
    #[serde(default = "default_allowed_origins")]
    pub allowed_origins: Vec<String>,
    
    /// Allowed methods
    #[serde(default = "default_allowed_methods")]
    pub allowed_methods: Vec<String>,
    
    /// Allowed headers
    #[serde(default = "default_allowed_headers")]
    pub allowed_headers: Vec<String>,
    
    /// Whether to allow credentials
    #[serde(default)]
    pub allow_credentials: bool,
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Requests per minute per IP
    #[serde(default = "default_requests_per_minute")]
    pub requests_per_minute: u32,
    
    /// Burst size
    #[serde(default = "default_burst_size")]
    pub burst_size: u32,
    
    /// Whether rate limiting is enabled
    #[serde(default = "default_rate_limit_enabled")]
    pub enabled: bool,
}

/// Performance tuning configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Number of worker threads
    #[serde(default = "default_worker_threads")]
    pub worker_threads: u32,
    
    /// Transaction batch size
    #[serde(default = "default_batch_size")]
    pub batch_size: u32,
    
    /// Simulation cache size
    #[serde(default = "default_cache_size")]
    pub simulation_cache_size: u32,
    
    /// Cache TTL in seconds
    #[serde(default = "default_cache_ttl")]
    pub cache_ttl_seconds: u64,
    
    /// Whether to enable metrics collection
    #[serde(default = "default_metrics_enabled")]
    pub metrics_enabled: bool,
}

// Default value functions
fn default_commitment() -> String { "confirmed".to_string() }
fn default_rpc_timeout() -> u64 { 30 }
fn default_max_retries() -> u32 { 3 }
fn default_pool_size() -> u32 { 10 }
fn default_max_compute_units() -> u32 { 1_400_000 }
fn default_max_fee_lamports() -> u64 { 100_000 }
fn default_validate_instructions() -> bool { true }
fn default_max_bundle_size() -> u32 { 5 }
fn default_require_simulation() -> bool { true }
fn default_max_writable_accounts() -> u32 { 64 }
fn default_parallel_signing() -> bool { true }
fn default_log_level() -> String { "info".to_string() }
fn default_log_format() -> String { "pretty".to_string() }
fn default_include_timestamps() -> bool { true }
fn default_bind_address() -> String { "127.0.0.1".to_string() }
fn default_port() -> u16 { 8080 }
fn default_request_timeout() -> u64 { 30 }
fn default_max_body_size() -> u64 { 1024 * 1024 } // 1MB
fn default_allowed_origins() -> Vec<String> { vec!["*".to_string()] }
fn default_allowed_methods() -> Vec<String> { vec!["GET".to_string(), "POST".to_string()] }
fn default_allowed_headers() -> Vec<String> { vec!["Content-Type".to_string(), "Authorization".to_string()] }
fn default_requests_per_minute() -> u32 { 60 }
fn default_burst_size() -> u32 { 10 }
fn default_rate_limit_enabled() -> bool { true }
fn default_worker_threads() -> u32 { num_cpus::get() as u32 }
fn default_batch_size() -> u32 { 10 }
fn default_cache_size() -> u32 { 1000 }
fn default_cache_ttl() -> u64 { 300 } // 5 minutes
fn default_metrics_enabled() -> bool { true }

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
                pool_size: default_pool_size(),
            },
            fees: FeeStrategy::default(),
            security: SecurityConfig {
                max_compute_units: default_max_compute_units(),
                max_fee_lamports: default_max_fee_lamports(),
                program_whitelist: vec![
                    system_program(),
                ],
                validate_instructions: default_validate_instructions(),
                max_bundle_size: default_max_bundle_size(),
                require_simulation: default_require_simulation(),
                max_writable_accounts: default_max_writable_accounts(),
            },
            signing: SigningConfig {
                fee_payer: SignerConfig {
                    signer_type: bundler_types::SignerType::Env {
                        var_name: "SOLANA_PRIVATE_KEY".to_string(),
                    },
                    alias: Some("fee_payer".to_string()),
                },
                additional_signers: vec![],
                parallel_signing: default_parallel_signing(),
            },
            jito: None,
            logging: LoggingConfig {
                level: default_log_level(),
                format: default_log_format(),
                file_enabled: false,
                file_path: None,
                include_timestamps: default_include_timestamps(),
            },
            service: ServiceConfig {
                bind_address: default_bind_address(),
                port: default_port(),
                request_timeout_seconds: default_request_timeout(),
                max_body_size_bytes: default_max_body_size(),
                cors: CorsConfig {
                    allowed_origins: default_allowed_origins(),
                    allowed_methods: default_allowed_methods(),
                    allowed_headers: default_allowed_headers(),
                    allow_credentials: false,
                },
                rate_limit: RateLimitConfig {
                    requests_per_minute: default_requests_per_minute(),
                    burst_size: default_burst_size(),
                    enabled: default_rate_limit_enabled(),
                },
            },
            performance: PerformanceConfig {
                worker_threads: default_worker_threads(),
                batch_size: default_batch_size(),
                simulation_cache_size: default_cache_size(),
                cache_ttl_seconds: default_cache_ttl(),
                metrics_enabled: default_metrics_enabled(),
            },
        }
    }
}

impl BundlerConfig {
    /// Load configuration from file
    pub fn load_from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        let config_str = std::fs::read_to_string(path.as_ref())
            .with_context(|| format!("Failed to read config file: {:?}", path.as_ref()))?;
        
        let config: BundlerConfig = toml::from_str(&config_str)
            .with_context(|| "Failed to parse config file")?;
        
        config.validate()?;
        Ok(config)
    }
    
    /// Load configuration from environment and files
    pub fn load() -> Result<Self> {
        let config = Config::builder()
            .add_source(File::with_name("bundler.config").required(false))
            .add_source(File::with_name("/etc/bundler/config").required(false))
            .add_source(Environment::with_prefix("BUNDLER").separator("_"))
            .build()
            .context("Failed to build configuration")?;
        
        let bundler_config: BundlerConfig = config.try_deserialize()
            .context("Failed to deserialize configuration")?;
        
        bundler_config.validate()?;
        Ok(bundler_config)
    }
    
    /// Save configuration to file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let config_str = toml::to_string_pretty(self)
            .context("Failed to serialize configuration")?;
        
        std::fs::write(path.as_ref(), config_str)
            .with_context(|| format!("Failed to write config file: {:?}", path.as_ref()))?;
        
        Ok(())
    }
    
    /// Validate configuration
    pub fn validate(&self) -> BundlerResult<()> {
        // Validate RPC endpoints
        if self.rpc.endpoints.is_empty() {
            return Err(BundlerError::Config("At least one RPC endpoint is required".to_string()));
        }
        
        for endpoint in &self.rpc.endpoints {
            if endpoint.url.is_empty() {
                return Err(BundlerError::Config("RPC endpoint URL cannot be empty".to_string()));
            }
            if endpoint.weight == 0 {
                return Err(BundlerError::Config("RPC endpoint weight must be greater than 0".to_string()));
            }
        }
        
        // Validate security settings
        if self.security.max_compute_units == 0 {
            return Err(BundlerError::Config("Max compute units must be greater than 0".to_string()));
        }
        
        if self.security.max_fee_lamports == 0 {
            return Err(BundlerError::Config("Max fee lamports must be greater than 0".to_string()));
        }
        
        if self.security.max_bundle_size == 0 {
            return Err(BundlerError::Config("Max bundle size must be greater than 0".to_string()));
        }

        if self.security.max_writable_accounts == 0 {
            return Err(BundlerError::Config(
                "Max writable accounts must be greater than 0".to_string(),
            ));
        }
        
        // Validate service settings
        if self.service.port == 0 {
            return Err(BundlerError::Config("Service port must be greater than 0".to_string()));
        }
        
        if self.service.request_timeout_seconds == 0 {
            return Err(BundlerError::Config("Request timeout must be greater than 0".to_string()));
        }
        
        // Validate performance settings
        if self.performance.worker_threads == 0 {
            return Err(BundlerError::Config("Worker threads must be greater than 0".to_string()));
        }
        
        if self.performance.batch_size == 0 {
            return Err(BundlerError::Config("Batch size must be greater than 0".to_string()));
        }
        
        Ok(())
    }
    
    /// Get primary RPC endpoint
    pub fn primary_rpc_endpoint(&self) -> &RpcEndpoint {
        self.rpc.endpoints
            .iter()
            .max_by_key(|endpoint| endpoint.weight)
            .unwrap_or(&self.rpc.endpoints[0])
    }
    
    /// Get RPC endpoints sorted by weight (descending)
    pub fn rpc_endpoints_by_weight(&self) -> Vec<&RpcEndpoint> {
        let mut endpoints: Vec<&RpcEndpoint> = self.rpc.endpoints.iter().collect();
        endpoints.sort_by(|a, b| b.weight.cmp(&a.weight));
        endpoints
    }
    
    /// Check if a program is whitelisted
    pub fn is_program_whitelisted(&self, program_id: &Pubkey) -> bool {
        self.security.is_program_whitelisted(program_id)
    }
    
    /// Get effective log level
    pub fn effective_log_level(&self) -> tracing::Level {
        match self.logging.level.to_lowercase().as_str() {
            "trace" => tracing::Level::TRACE,
            "debug" => tracing::Level::DEBUG,
            "info" => tracing::Level::INFO,
            "warn" => tracing::Level::WARN,
            "error" => tracing::Level::ERROR,
            _ => tracing::Level::INFO,
        }
    }
}

/// Builder for [`SecurityConfig`]
pub struct SecurityConfigBuilder {
    config: SecurityConfig,
}

impl SecurityConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: SecurityConfig::default(),
        }
    }

    pub fn with_program_whitelist(mut self, programs: Vec<Pubkey>) -> Self {
        self.config.program_whitelist = programs;
        self
    }

    pub fn with_max_compute_units(mut self, max_compute_units: u32) -> Self {
        self.config.max_compute_units = max_compute_units;
        self
    }

    pub fn with_max_fee_lamports(mut self, max_fee_lamports: u64) -> Self {
        self.config.max_fee_lamports = max_fee_lamports;
        self
    }

    pub fn with_validate_instructions(mut self, validate: bool) -> Self {
        self.config.validate_instructions = validate;
        self
    }

    pub fn with_max_bundle_size(mut self, max_bundle_size: u32) -> Self {
        self.config.max_bundle_size = max_bundle_size;
        self
    }

    pub fn with_require_simulation(mut self, require_simulation: bool) -> Self {
        self.config.require_simulation = require_simulation;
        self
    }

    pub fn with_max_writable_accounts(mut self, max_writable_accounts: u32) -> Self {
        self.config.max_writable_accounts = max_writable_accounts;
        self
    }

    pub fn build(self) -> BundlerResult<SecurityConfig> {
        if self.config.max_writable_accounts == 0 {
            return Err(BundlerError::Config("max_writable_accounts must be greater than 0".to_string()));
        }

        Ok(self.config)
    }
}

impl Default for SecurityConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration builder for programmatic construction
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
    
    pub fn with_port(mut self, port: u16) -> Self {
        self.config.service.port = port;
        self
    }
    
    pub fn with_worker_threads(mut self, threads: u32) -> Self {
        self.config.performance.worker_threads = threads;
        self
    }
    
    pub fn build(self) -> Result<BundlerConfig> {
        self.config.validate()?;
        Ok(self.config)
    }
}

impl Default for BundlerConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper function to get system program ID
pub fn system_program() -> Pubkey {
    solana_system_program::id()
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
    fn test_security_config_builder() {
        let program_id = Pubkey::new_unique();
        let security = SecurityConfigBuilder::new()
            .with_program_whitelist(vec![program_id])
            .with_max_writable_accounts(10)
            .with_require_simulation(false)
            .build()
            .unwrap();

        assert_eq!(security.max_writable_accounts, 10);
        assert!(!security.require_simulation);
        assert!(security.is_program_whitelisted(&program_id));
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
        assert!(config.is_program_whitelisted(&system_program()));
        
        // Random program should not be whitelisted
        let random_program = Pubkey::new_unique();
        assert!(!config.is_program_whitelisted(&random_program));
    }
    
    #[test]
    fn test_rpc_endpoint_selection() {
        let mut config = BundlerConfig::default();
        config.rpc.endpoints = vec![
            RpcEndpoint {
                url: "https://low-weight.com".to_string(),
                weight: 10,
                supports_jito: false,
                auth_token: None,
            },
            RpcEndpoint {
                url: "https://high-weight.com".to_string(),
                weight: 100,
                supports_jito: true,
                auth_token: Some("token".to_string()),
            },
        ];
        
        let primary = config.primary_rpc_endpoint();
        assert_eq!(primary.url, "https://high-weight.com");
        assert_eq!(primary.weight, 100);
        
        let sorted = config.rpc_endpoints_by_weight();
        assert_eq!(sorted[0].weight, 100);
        assert_eq!(sorted[1].weight, 10);
    }
    
    #[test]
    fn test_config_validation_errors() {
        let mut config = BundlerConfig::default();
        
        // Test empty RPC endpoints
        config.rpc.endpoints.clear();
        assert!(config.validate().is_err());
        
        // Reset and test zero compute units
        config = BundlerConfig::default();
        config.security.max_compute_units = 0;
        assert!(config.validate().is_err());
        
        // Reset and test zero port
        config = BundlerConfig::default();
        config.service.port = 0;
        assert!(config.validate().is_err());
    }
    
    #[test]
    fn test_log_level_parsing() {
        let mut config = BundlerConfig::default();
        
        config.logging.level = "trace".to_string();
        assert_eq!(config.effective_log_level(), tracing::Level::TRACE);
        
        config.logging.level = "debug".to_string();
        assert_eq!(config.effective_log_level(), tracing::Level::DEBUG);
        
        config.logging.level = "info".to_string();
        assert_eq!(config.effective_log_level(), tracing::Level::INFO);
        
        config.logging.level = "warn".to_string();
        assert_eq!(config.effective_log_level(), tracing::Level::WARN);
        
        config.logging.level = "error".to_string();
        assert_eq!(config.effective_log_level(), tracing::Level::ERROR);
        
        config.logging.level = "invalid".to_string();
        assert_eq!(config.effective_log_level(), tracing::Level::INFO);
    }
    
    #[test]
    fn test_cors_config() {
        let config = BundlerConfig::default();
        
        assert_eq!(config.service.cors.allowed_origins, vec!["*"]);
        assert_eq!(config.service.cors.allowed_methods, vec!["GET", "POST"]);
        assert!(!config.service.cors.allow_credentials);
    }
    
    #[test]
    fn test_rate_limit_config() {
        let config = BundlerConfig::default();
        
        assert_eq!(config.service.rate_limit.requests_per_minute, 60);
        assert_eq!(config.service.rate_limit.burst_size, 10);
        assert!(config.service.rate_limit.enabled);
    }
    
    #[test]
    fn test_performance_config() {
        let config = BundlerConfig::default();
        
        assert!(config.performance.worker_threads > 0);
        assert_eq!(config.performance.batch_size, 10);
        assert_eq!(config.performance.simulation_cache_size, 1000);
        assert_eq!(config.performance.cache_ttl_seconds, 300);
        assert!(config.performance.metrics_enabled);
    }
    
    #[test]
    fn test_jito_config_integration() {
        let jito_config = JitoConfig {
            block_engine_url: "https://mainnet.block-engine.jito.wtf".to_string(),
            relayer_url: "https://mainnet.relayer.jito.wtf".to_string(),
            auth_keypair_path: Some("/path/to/keypair.json".to_string()),
            tip_lamports: 10000,
            max_tip_lamports: 100000,
            enabled: true,
        };
        
        let config = BundlerConfigBuilder::new()
            .with_jito_config(jito_config.clone())
            .build()
            .unwrap();
        
        assert!(config.jito.is_some());
        let jito = config.jito.unwrap();
        assert_eq!(jito.block_engine_url, jito_config.block_engine_url);
        assert_eq!(jito.tip_lamports, jito_config.tip_lamports);
        assert!(jito.enabled);
    }
    
    #[test]
    fn test_security_config_validation() {
        let mut config = BundlerConfig::default();
        
        // Test max bundle size validation
        config.security.max_bundle_size = 0;
        assert!(config.validate().is_err());

        // Test max fee validation
        config = BundlerConfig::default();
        config.security.max_fee_lamports = 0;
        assert!(config.validate().is_err());

        // Test writable account validation
        config = BundlerConfig::default();
        config.security.max_writable_accounts = 0;
        assert!(config.validate().is_err());
    }
    
    #[test]
    fn test_empty_program_whitelist() {
        let mut config = BundlerConfig::default();
        config.security.program_whitelist.clear();
        
        // Empty whitelist should allow all programs
        let random_program = Pubkey::new_unique();
        assert!(config.is_program_whitelisted(&random_program));
    }
    
    #[test]
    fn test_builder_pattern() {
        let config = BundlerConfigBuilder::new()
            .with_rpc_endpoint("https://api.devnet.solana.com".to_string(), 50)
            .with_rpc_endpoint("https://api.testnet.solana.com".to_string(), 30)
            .with_log_level("trace".to_string())
            .with_port(9090)
            .with_worker_threads(8)
            .build()
            .unwrap();
        
        assert_eq!(config.rpc.endpoints.len(), 3); // 2 added + 1 default
        assert_eq!(config.logging.level, "trace");
        assert_eq!(config.service.port, 9090);
        assert_eq!(config.performance.worker_threads, 8);
    }
    
    #[test]
    fn test_config_defaults() {
        let config = BundlerConfig::default();
        
        // Test RPC defaults
        assert_eq!(config.rpc.commitment, "confirmed");
        assert_eq!(config.rpc.timeout_seconds, 30);
        assert_eq!(config.rpc.max_retries, 3);
        
        // Test service defaults
        assert_eq!(config.service.bind_address, "127.0.0.1");
        assert_eq!(config.service.port, 8080);
        assert_eq!(config.service.request_timeout_seconds, 30);
        
        // Test logging defaults
        assert_eq!(config.logging.level, "info");
        assert_eq!(config.logging.format, "pretty");
        assert!(config.logging.include_timestamps);
        assert!(!config.logging.file_enabled);

        // Test security defaults
        assert!(config.security.require_simulation);
        assert_eq!(config.security.max_writable_accounts, 64);
    }
}
