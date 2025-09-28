use base64::{engine::general_purpose, Engine};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::Signature,
};
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

// Type aliases for clarity
pub type RequestId = Uuid;
pub type ComputeUnits = u32;
pub type Lamports = u64;

/// Maximum compute units per transaction
pub const MAX_COMPUTE_UNITS: ComputeUnits = 1_400_000;

/// Maximum compute unit price (micro-lamports per CU)
pub const MAX_COMPUTE_UNIT_PRICE: u64 = 10_000; // 10k lamports per CU max

/// RPC endpoint configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcEndpoint {
    /// RPC endpoint URL
    pub url: String,
    
    /// Weight for load balancing (higher = more traffic)
    pub weight: u32,
    
    /// Whether this endpoint supports Jito bundles
    pub supports_jito: bool,
    
    /// Optional authentication token
    pub auth_token: Option<String>,
}

/// Jito configuration for bundle submission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JitoConfig {
    /// Jito block engine URL
    pub block_engine_url: String,
    
    /// Jito relayer URL
    pub relayer_url: String,
    
    /// Authentication keypair path
    pub auth_keypair_path: Option<String>,
    
    /// Tip amount in lamports
    pub tip_lamports: u64,
    
    /// Maximum tip amount in lamports
    pub max_tip_lamports: u64,
    
    /// Whether to use Jito for all transactions
    pub enabled: bool,
}

/// Bundle request from client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleRequest {
    /// Unique request identifier
    pub request_id: RequestId,
    
    /// Whether bundle should be atomic (all or nothing)
    pub atomic: bool,
    
    /// Compute budget configuration
    pub compute: ComputeConfig,
    
    /// Address lookup tables to use
    pub alt_tables: Vec<Pubkey>,
    
    /// Instructions to execute
    pub instructions: Vec<InstructionData>,
    
    /// Additional signers (beyond fee payer)
    pub signers: Vec<SignerConfig>,
    
    /// Optional metadata
    pub metadata: HashMap<String, String>,
}

/// Compute budget configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeConfig {
    /// Compute unit limit strategy
    pub limit: ComputeLimit,
    
    /// Compute unit price strategy
    pub price: ComputePrice,
    
    /// Maximum price willing to pay (lamports)
    pub max_price_lamports: Lamports,
}

/// Compute unit limit strategies
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum ComputeLimit {
    /// Automatically determine optimal limit
    Auto,
    /// Use fixed compute unit limit
    Fixed(ComputeUnits),
}

/// Compute unit price strategies
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum ComputePrice {
    /// Automatically determine optimal price
    Auto,
    /// Use fixed price (micro-lamports per CU)
    Fixed(u64),
}

/// Instruction data for bundle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstructionData {
    /// Program ID to invoke
    pub program_id: Pubkey,
    
    /// Account metadata
    pub keys: Vec<AccountMeta>,
    
    /// Instruction data (base64 encoded)
    pub data_b64: String,
}

/// Signer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignerConfig {
    /// Type of signer
    #[serde(flatten)]
    pub signer_type: SignerType,
    
    /// Optional alias for the signer
    pub alias: Option<String>,
}

/// Types of signers supported
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SignerType {
    /// File-based keypair
    File { path: String },
    
    /// Environment variable containing private key
    Env { var_name: String },
    
    /// AWS KMS key
    Kms { 
        key_id: String,
        region: Option<String>,
    },
    
    /// Hardware wallet (future)
    Hardware { device_path: String },
}

/// Bundle response to client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleResponse {
    /// Original request ID
    pub request_id: RequestId,
    
    /// Bundle processing status
    pub status: BundleStatus,
    
    /// Individual transaction results
    pub transactions: Vec<TransactionResult>,
    
    /// Bundle signature (if successful)
    pub bundle_signature: Option<Signature>,
    
    /// Slot where bundle was included
    pub slot: Option<u64>,
    
    /// Blockhash used
    pub blockhash: Option<String>,
    
    /// Confirmation status
    pub confirmation: ConfirmationStatus,
    
    /// URL to view logs
    pub logs_url: Option<String>,
    
    /// Performance metrics
    pub metrics: BundleMetrics,
    
    /// Completion timestamp
    pub completed_at: DateTime<Utc>,
}

/// Bundle processing status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BundleStatus {
    /// Bundle is being processed
    Processing,
    
    /// Bundle completed successfully
    Success,
    
    /// Bundle failed
    Failed,
    
    /// Bundle timed out
    Timeout,
    
    /// Bundle was rejected
    Rejected,
}

/// Individual transaction result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionResult {
    /// Transaction signature (if successful)
    pub signature: Option<Signature>,
    
    /// Transaction status
    pub status: TransactionStatus,
    
    /// Error message (if failed)
    pub error: Option<String>,
    
    /// Compute units consumed
    pub compute_units_consumed: Option<ComputeUnits>,
    
    /// Fee paid in lamports
    pub fee_paid_lamports: Option<Lamports>,
    
    /// Transaction logs
    pub logs: Vec<String>,
}

/// Transaction status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TransactionStatus {
    /// Transaction is pending
    Pending,
    
    /// Transaction was processed
    Processed,
    
    /// Transaction was confirmed
    Confirmed,
    
    /// Transaction was finalized
    Finalized,
    
    /// Transaction failed
    Failed,
}

/// Confirmation status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmationStatus {
    /// Not yet processed
    Unprocessed,
    
    /// Processed but not confirmed
    Processed,
    
    /// Confirmed by cluster
    Confirmed,
    
    /// Finalized by cluster
    Finalized,
}

/// Bundle performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleMetrics {
    /// Total number of transactions in bundle
    pub total_transactions: u32,
    
    /// Number of successful transactions
    pub successful_transactions: u32,
    
    /// Number of failed transactions
    pub failed_transactions: u32,
    
    /// Total compute units used
    pub total_compute_units: ComputeUnits,
    
    /// Total fee paid in lamports
    pub total_fee_paid: Lamports,
    
    /// Total processing time in milliseconds
    pub processing_time_ms: u64,
    
    /// Total latency from request to completion
    pub total_latency_ms: u64,
    
    /// Time spent in simulation
    pub simulation_time_ms: u64,
    
    /// Time spent signing transactions
    pub signing_time_ms: u64,
    
    /// Time spent submitting to network
    pub submission_time_ms: u64,
    
    /// Time spent waiting for confirmation
    pub confirmation_time_ms: u64,
    
    /// Number of retry attempts
    pub retry_attempts: u32,
    
    /// RPC endpoints used
    pub rpc_endpoints_used: Vec<String>,
}

/// Fee calculation strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeStrategy {
    /// Base fee in lamports
    pub base_fee_lamports: Lamports,
    
    /// Priority fee in lamports
    pub priority_fee_lamports: Lamports,
    
    /// Compute unit price in micro-lamports
    pub compute_unit_price_micro_lamports: u64,
    
    /// Maximum total price willing to pay
    pub max_price_lamports: Lamports,
    
    /// Percentile to use for fee estimation (0-100)
    pub base_percentile: u8,
    
    /// Buffer percentage to add to estimated fees
    pub buffer_percent: u8,
    
    /// Whether to use adaptive fee calculation
    pub adaptive: bool,
    
    /// Whether to enable fee bumping on failure
    pub enable_bump: bool,
    
    /// Maximum number of bump attempts
    pub max_bump_attempts: u32,
}

impl Default for FeeStrategy {
    fn default() -> Self {
        Self {
            base_fee_lamports: 5000,
            priority_fee_lamports: 1000,
            compute_unit_price_micro_lamports: 1000,
            max_price_lamports: 100000,
            base_percentile: 75,
            buffer_percent: 20,
            adaptive: true,
            enable_bump: true,
            max_bump_attempts: 3,
        }
    }
}

/// Health check status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    /// Overall health status
    pub healthy: bool,
    
    /// Component-specific health
    pub components: HashMap<String, ComponentHealth>,
    
    /// Timestamp of health check
    pub timestamp: DateTime<Utc>,
}

/// Individual component health
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    /// Whether component is healthy
    pub healthy: bool,
    
    /// Optional status message
    pub message: Option<String>,
    
    /// Last successful operation timestamp
    pub last_success: Option<DateTime<Utc>>,
}

/// Bundler-specific errors
#[derive(Debug, Error)]
pub enum BundlerError {
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("RPC error: {0}")]
    Rpc(String),
    
    #[error("Signing error: {0}")]
    Signing(String),
    
    #[error("Simulation error: {0}")]
    Simulation(String),
    
    #[error("Transaction error: {0}")]
    Transaction(String),
    
    #[error("Timeout error: {0}")]
    Timeout(String),
    
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    
    #[error("Invalid instruction: {0}")]
    InvalidInstruction(String),
    
    #[error("RPC error: {0}")]
    RpcError(String),
    
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result type for bundler operations
pub type BundlerResult<T> = Result<T, BundlerError>;

// Conversion implementations
impl From<InstructionData> for Instruction {
    fn from(data: InstructionData) -> Self {
        let instruction_data = general_purpose::STANDARD
            .decode(&data.data_b64)
            .unwrap_or_default();
        
        Instruction {
            program_id: data.program_id,
            accounts: data.keys,
            data: instruction_data,
        }
    }
}

impl From<Instruction> for InstructionData {
    fn from(instruction: Instruction) -> Self {
        Self {
            program_id: instruction.program_id,
            keys: instruction.accounts,
            data_b64: general_purpose::STANDARD.encode(&instruction.data),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_bundle_request_serialization() {
        let request = BundleRequest {
            request_id: Uuid::new_v4(),
            atomic: true,
            compute: ComputeConfig {
                limit: ComputeLimit::Fixed(200000),
                price: ComputePrice::Fixed(1000),
                max_price_lamports: 50000,
            },
            alt_tables: vec![],
            instructions: vec![
                InstructionData {
                    program_id: Pubkey::new_unique(),
                    keys: vec![
                        AccountMeta {
                            pubkey: Pubkey::new_unique(),
                            is_signer: true,
                            is_writable: true,
                        }
                    ],
                    data_b64: general_purpose::STANDARD.encode(&[1, 2, 3, 4]),
                }
            ],
            signers: vec![],
            metadata: HashMap::new(),
        };

        // Test serialization
        let json = serde_json::to_string(&request).expect("Should serialize");
        assert!(!json.is_empty());

        // Test deserialization
        let deserialized: BundleRequest = serde_json::from_str(&json).expect("Should deserialize");
        assert_eq!(request.request_id, deserialized.request_id);
        assert_eq!(request.atomic, deserialized.atomic);
        assert_eq!(request.compute.limit, deserialized.compute.limit);
        assert_eq!(request.compute.price, deserialized.compute.price);
    }

    #[test]
    fn test_bundle_response_complete() {
        let response = BundleResponse {
            request_id: Uuid::new_v4(),
            status: BundleStatus::Success,
            transactions: vec![
                TransactionResult {
                    signature: Some(Signature::new_unique()),
                    status: TransactionStatus::Finalized,
                    error: None,
                    compute_units_consumed: Some(150000),
                    fee_paid_lamports: Some(5000),
                    logs: vec!["Program log: Success".to_string()],
                }
            ],
            bundle_signature: Some(Signature::new_unique()),
            slot: Some(12345),
            blockhash: Some("ABC123".to_string()),
            confirmation: ConfirmationStatus::Finalized,
            logs_url: Some("https://logs.example.com/bundle123".to_string()),
            metrics: BundleMetrics {
                total_transactions: 1,
                successful_transactions: 1,
                failed_transactions: 0,
                total_compute_units: 150000,
                total_fee_paid: 5000,
                processing_time_ms: 1500,
                total_latency_ms: 1500,
                simulation_time_ms: 200,
                signing_time_ms: 100,
                submission_time_ms: 800,
                confirmation_time_ms: 400,
                retry_attempts: 0,
                rpc_endpoints_used: vec!["https://api.mainnet-beta.solana.com".to_string()],
            },
            completed_at: Utc::now(),
        };

        // Test serialization
        let json = serde_json::to_string(&response).expect("Should serialize");
        assert!(!json.is_empty());

        // Test deserialization
        let deserialized: BundleResponse = serde_json::from_str(&json).expect("Should deserialize");
        assert_eq!(response.request_id, deserialized.request_id);
        assert_eq!(response.status, deserialized.status);
        assert_eq!(response.transactions.len(), deserialized.transactions.len());
    }

    #[test]
    fn test_instruction_data_conversion() {
        let instruction_data = InstructionData {
            program_id: Pubkey::new_unique(),
            keys: vec![
                AccountMeta {
                    pubkey: Pubkey::new_unique(),
                    is_signer: true,
                    is_writable: false,
                }
            ],
            data_b64: general_purpose::STANDARD.encode(&[1, 2, 3, 4]),
        };

        // Test conversion to Solana Instruction
        let solana_instruction: Instruction = instruction_data.clone().into();
        assert_eq!(instruction_data.program_id, solana_instruction.program_id);
        assert_eq!(instruction_data.keys, solana_instruction.accounts);
        
        let expected_data = general_purpose::STANDARD.decode(&instruction_data.data_b64).unwrap();
        assert_eq!(expected_data, solana_instruction.data);

        // Test round-trip conversion
        let back_to_data: InstructionData = solana_instruction.into();
        assert_eq!(instruction_data.program_id, back_to_data.program_id);
        assert_eq!(instruction_data.keys, back_to_data.keys);
        assert_eq!(instruction_data.data_b64, back_to_data.data_b64);
    }

    #[test]
    fn test_compute_config_variants() {
        // Test Auto variants
        let auto_config = ComputeConfig {
            limit: ComputeLimit::Auto,
            price: ComputePrice::Auto,
            max_price_lamports: 100000,
        };

        let json = serde_json::to_string(&auto_config).expect("Should serialize");
        let deserialized: ComputeConfig = serde_json::from_str(&json).expect("Should deserialize");
        assert_eq!(auto_config.limit, deserialized.limit);
        assert_eq!(auto_config.price, deserialized.price);

        // Test Fixed variants
        let fixed_config = ComputeConfig {
            limit: ComputeLimit::Fixed(400000),
            price: ComputePrice::Fixed(2000),
            max_price_lamports: 80000,
        };

        let json = serde_json::to_string(&fixed_config).expect("Should serialize");
        let deserialized: ComputeConfig = serde_json::from_str(&json).expect("Should deserialize");
        assert_eq!(fixed_config.limit, deserialized.limit);
        assert_eq!(fixed_config.price, deserialized.price);
        assert_eq!(fixed_config.max_price_lamports, deserialized.max_price_lamports);
    }

    #[test]
    fn test_fee_strategy_default() {
        let strategy = FeeStrategy::default();
        
        assert_eq!(strategy.base_fee_lamports, 5000);
        assert_eq!(strategy.priority_fee_lamports, 1000);
        assert_eq!(strategy.compute_unit_price_micro_lamports, 1000);
        assert_eq!(strategy.max_price_lamports, 100000);
        assert_eq!(strategy.base_percentile, 75);
        assert_eq!(strategy.buffer_percent, 20);
        assert!(strategy.adaptive);
        assert!(strategy.enable_bump);
        assert_eq!(strategy.max_bump_attempts, 3);

        // Test that max_price is enforced
        assert!(strategy.max_price_lamports > strategy.base_fee_lamports);
        assert!(strategy.max_price_lamports > strategy.priority_fee_lamports);
    }

    #[test]
    fn test_bundle_metrics_calculations() {
        let metrics = BundleMetrics {
            total_transactions: 5,
            successful_transactions: 4,
            failed_transactions: 1,
            total_compute_units: 1000000,
            total_fee_paid: 25000,
            processing_time_ms: 2500,
            total_latency_ms: 2500,
            simulation_time_ms: 400,
            signing_time_ms: 200,
            submission_time_ms: 1200,
            confirmation_time_ms: 700,
            retry_attempts: 2,
            rpc_endpoints_used: vec!["rpc1".to_string(), "rpc2".to_string()],
        };

        // Test basic calculations
        assert_eq!(metrics.total_transactions, metrics.successful_transactions + metrics.failed_transactions);
        assert!(metrics.processing_time_ms > 0);
        assert!(metrics.total_latency_ms >= metrics.processing_time_ms);
        
        // Test serialization
        let json = serde_json::to_string(&metrics).expect("Should serialize");
        let deserialized: BundleMetrics = serde_json::from_str(&json).expect("Should deserialize");
        assert_eq!(metrics.total_transactions, deserialized.total_transactions);
        assert_eq!(metrics.total_compute_units, deserialized.total_compute_units);
    }

    #[test]
    fn test_health_status() {
        let health = HealthStatus {
            healthy: true,
            components: HashMap::from([
                ("rpc".to_string(), ComponentHealth {
                    healthy: true,
                    message: Some("Connected".to_string()),
                    last_success: Some(Utc::now()),
                }),
                ("signer".to_string(), ComponentHealth {
                    healthy: true,
                    message: None,
                    last_success: Some(Utc::now() - Duration::minutes(5)),
                })
            ]),
            timestamp: Utc::now(),
        };

        assert!(health.healthy);
        assert!(health.components.contains_key("rpc"));
        assert!(health.components.contains_key("signer"));
        
        let rpc_health = &health.components["rpc"];
        assert!(rpc_health.healthy);
        assert!(rpc_health.message.is_some());
        assert!(rpc_health.last_success.is_some());
    }

    #[test]
    fn test_component_health_unhealthy() {
        let component = ComponentHealth {
            healthy: false,
            message: Some("Connection timeout".to_string()),
            last_success: Some(Utc::now() - Duration::hours(1)),
        };

        assert!(!component.healthy);
        assert!(component.message.is_some());
        assert!(component.last_success.is_some());
        
        // Test serialization
        let json = serde_json::to_string(&component).expect("Should serialize");
        let deserialized: ComponentHealth = serde_json::from_str(&json).expect("Should deserialize");
        assert_eq!(component.healthy, deserialized.healthy);
        assert_eq!(component.message, deserialized.message);
    }

    #[test]
    fn test_signer_config_variants() {
        // Test File signer
        let file_signer = SignerConfig {
            signer_type: SignerType::File { 
                path: "/path/to/keypair.json".to_string() 
            },
            alias: Some("main".to_string()),
        };

        let json = serde_json::to_string(&file_signer).expect("Should serialize");
        let deserialized: SignerConfig = serde_json::from_str(&json).expect("Should deserialize");
        assert!(matches!(deserialized.signer_type, SignerType::File { .. }));
        assert_eq!(file_signer.alias, deserialized.alias);

        // Test KMS signer
        let kms_signer = SignerConfig {
            signer_type: SignerType::Kms {
                key_id: "arn:aws:kms:us-east-1:123456789012:key/12345678-1234-1234-1234-123456789012".to_string(),
                region: Some("us-east-1".to_string()),
            },
            alias: Some("production".to_string()),
        };

        let json = serde_json::to_string(&kms_signer).expect("Should serialize");
        let deserialized: SignerConfig = serde_json::from_str(&json).expect("Should deserialize");
        assert!(matches!(deserialized.signer_type, SignerType::Kms { .. }));

        // Test Env signer
        let env_signer = SignerConfig {
            signer_type: SignerType::Env {
                var_name: "SOLANA_PRIVATE_KEY".to_string(),
            },
            alias: None,
        };

        let json = serde_json::to_string(&env_signer).expect("Should serialize");
        let deserialized: SignerConfig = serde_json::from_str(&json).expect("Should deserialize");
        assert!(matches!(deserialized.signer_type, SignerType::Env { .. }));
    }

    #[test]
    fn test_bundle_status_variants() {
        let statuses = vec![
            BundleStatus::Processing,
            BundleStatus::Success,
            BundleStatus::Failed,
            BundleStatus::Timeout,
            BundleStatus::Rejected,
        ];

        for status in statuses {
            let json = serde_json::to_string(&status).expect("Should serialize");
            let deserialized: BundleStatus = serde_json::from_str(&json).expect("Should deserialize");
            assert_eq!(status, deserialized);
        }
    }

    #[test]
    fn test_transaction_status_variants() {
        let statuses = vec![
            TransactionStatus::Pending,
            TransactionStatus::Processed,
            TransactionStatus::Confirmed,
            TransactionStatus::Finalized,
            TransactionStatus::Failed,
        ];

        for status in statuses {
            let json = serde_json::to_string(&status).expect("Should serialize");
            let deserialized: TransactionStatus = serde_json::from_str(&json).expect("Should deserialize");
            assert_eq!(status, deserialized);
        }
    }

    #[test]
    fn test_confirmation_status_variants() {
        let statuses = vec![
            ConfirmationStatus::Unprocessed,
            ConfirmationStatus::Processed,
            ConfirmationStatus::Confirmed,
            ConfirmationStatus::Finalized,
        ];

        for status in statuses {
            let json = serde_json::to_string(&status).expect("Should serialize");
            let deserialized: ConfirmationStatus = serde_json::from_str(&json).expect("Should deserialize");
            assert_eq!(status, deserialized);
        }
    }

    #[test]
    fn test_bundler_error_variants() {
        let errors = vec![
            BundlerError::Config("Invalid config".to_string()),
            BundlerError::Rpc("Connection failed".to_string()),
            BundlerError::Signing("Key not found".to_string()),
            BundlerError::Simulation("Simulation failed".to_string()),
            BundlerError::Transaction("Transaction failed".to_string()),
            BundlerError::Timeout("Request timeout".to_string()),
            BundlerError::InvalidInput("Bad input".to_string()),
            BundlerError::InvalidInstruction("Bad instruction".to_string()),
            BundlerError::RpcError("RPC error".to_string()),
            BundlerError::Internal("Internal error".to_string()),
        ];

        for error in errors {
            let error_string = error.to_string();
            assert!(!error_string.is_empty());
        }
    }

    #[test]
    fn test_rpc_endpoint_serialization() {
        let endpoint = RpcEndpoint {
            url: "https://api.mainnet-beta.solana.com".to_string(),
            weight: 100,
            supports_jito: false,
            auth_token: Some("secret_token".to_string()),
        };

        let json = serde_json::to_string(&endpoint).expect("Should serialize");
        let deserialized: RpcEndpoint = serde_json::from_str(&json).expect("Should deserialize");
        
        assert_eq!(endpoint.url, deserialized.url);
        assert_eq!(endpoint.weight, deserialized.weight);
        assert_eq!(endpoint.supports_jito, deserialized.supports_jito);
        assert_eq!(endpoint.auth_token, deserialized.auth_token);
    }

    #[test]
    fn test_jito_config_serialization() {
        let jito_config = JitoConfig {
            block_engine_url: "https://mainnet.block-engine.jito.wtf".to_string(),
            relayer_url: "https://mainnet.relayer.jito.wtf".to_string(),
            auth_keypair_path: Some("/path/to/keypair.json".to_string()),
            tip_lamports: 10000,
            max_tip_lamports: 100000,
            enabled: true,
        };

        let json = serde_json::to_string(&jito_config).expect("Should serialize");
        let deserialized: JitoConfig = serde_json::from_str(&json).expect("Should deserialize");
        
        assert_eq!(jito_config.block_engine_url, deserialized.block_engine_url);
        assert_eq!(jito_config.relayer_url, deserialized.relayer_url);
        assert_eq!(jito_config.auth_keypair_path, deserialized.auth_keypair_path);
        assert_eq!(jito_config.tip_lamports, deserialized.tip_lamports);
        assert_eq!(jito_config.max_tip_lamports, deserialized.max_tip_lamports);
        assert_eq!(jito_config.enabled, deserialized.enabled);
    }

    #[test]
    fn test_integration_scenario() {
        // Create a complete bundle request
        let request = BundleRequest {
            request_id: Uuid::new_v4(),
            atomic: true,
            compute: ComputeConfig {
                limit: ComputeLimit::Fixed(400000),
                price: ComputePrice::Fixed(2000),
                max_price_lamports: 80000,
            },
            alt_tables: vec![],
            instructions: vec![
                InstructionData {
                    program_id: Pubkey::new_unique(),
                    keys: vec![
                        AccountMeta {
                            pubkey: Pubkey::new_unique(),
                            is_signer: true,
                            is_writable: false,
                        },
                        AccountMeta {
                            pubkey: Pubkey::new_unique(),
                            is_signer: false,
                            is_writable: true,
                        },
                    ],
                    data_b64: general_purpose::STANDARD.encode(&[2, 0, 0, 0, 100, 0, 0, 0, 0, 0, 0, 0]), // Transfer instruction
                }
            ],
            signers: vec![],
            metadata: {
                let mut map = HashMap::new();
                map.insert("client".to_string(), "test-client".to_string());
                map.insert("version".to_string(), "1.0.0".to_string());
                map
            },
        };

        // Create a successful response
        let response = BundleResponse {
            request_id: request.request_id,
            status: BundleStatus::Success,
            transactions: vec![
                TransactionResult {
                    signature: Some(Signature::new_unique()),
                    status: TransactionStatus::Finalized,
                    error: None,
                    compute_units_consumed: Some(350000),
                    fee_paid_lamports: Some(8000),
                    logs: vec![
                        "Program log: Instruction: Transfer".to_string(),
                        "Program log: Success".to_string(),
                    ],
                }
            ],
            bundle_signature: Some(Signature::new_unique()),
            slot: Some(98765),
            blockhash: Some("XYZ789".to_string()),
            confirmation: ConfirmationStatus::Finalized,
            logs_url: Some("https://explorer.solana.com/tx/".to_string()),
            metrics: BundleMetrics {
                total_transactions: 1,
                successful_transactions: 1,
                failed_transactions: 0,
                total_compute_units: 350000,
                total_fee_paid: 8000,
                processing_time_ms: 1200,
                total_latency_ms: 1200,
                simulation_time_ms: 150,
                signing_time_ms: 50,
                submission_time_ms: 600,
                confirmation_time_ms: 400,
                retry_attempts: 0,
                rpc_endpoints_used: vec!["https://api.mainnet-beta.solana.com".to_string()],
            },
            completed_at: Utc::now(),
        };

        // Test full serialization cycle
        let request_json = serde_json::to_string(&request).expect("Request should serialize");
        let response_json = serde_json::to_string(&response).expect("Response should serialize");

        let deserialized_request: BundleRequest = serde_json::from_str(&request_json).expect("Request should deserialize");
        let deserialized_response: BundleResponse = serde_json::from_str(&response_json).expect("Response should deserialize");

        assert_eq!(request.request_id, deserialized_request.request_id);
        assert_eq!(response.request_id, deserialized_response.request_id);
        assert_eq!(request.request_id, response.request_id);
    }

    #[test]
    fn test_error_scenario() {
        // Create a failed bundle response
        let response = BundleResponse {
            request_id: Uuid::new_v4(),
            status: BundleStatus::Failed,
            transactions: vec![
                TransactionResult {
                    signature: None,
                    status: TransactionStatus::Failed,
                    error: Some("Insufficient funds".to_string()),
                    compute_units_consumed: Some(0),
                    fee_paid_lamports: Some(5000), // Fee still paid even on failure
                    logs: vec![
                        "Program log: Error: Insufficient funds".to_string(),
                    ],
                }
            ],
            bundle_signature: None,
            slot: None,
            blockhash: Some("ABC123".to_string()),
            confirmation: ConfirmationStatus::Unprocessed,
            logs_url: None,
            metrics: BundleMetrics {
                total_transactions: 1,
                successful_transactions: 0,
                failed_transactions: 1,
                total_compute_units: 0,
                total_fee_paid: 5000,
                processing_time_ms: 800,
                total_latency_ms: 800,
                simulation_time_ms: 100,
                signing_time_ms: 50,
                submission_time_ms: 650,
                confirmation_time_ms: 0,
                retry_attempts: 2,
                rpc_endpoints_used: vec!["https://api.mainnet-beta.solana.com".to_string()],
            },
            completed_at: Utc::now(),
        };

        // Test that error scenarios serialize correctly
        let json = serde_json::to_string(&response).expect("Should serialize");
        let deserialized: BundleResponse = serde_json::from_str(&json).expect("Should deserialize");

        assert_eq!(response.status, BundleStatus::Failed);
        assert_eq!(deserialized.status, BundleStatus::Failed);
        assert!(response.transactions[0].error.is_some());
        assert!(deserialized.transactions[0].error.is_some());
        assert_eq!(response.metrics.failed_transactions, 1);
        assert_eq!(response.metrics.successful_transactions, 0);
    }
}
