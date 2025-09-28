use base64::{engine::general_purpose, Engine};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_sdk::{
    instruction::Instruction,
    pubkey::Pubkey,
    signature::Signature,

};
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

/// Request ID for tracking bundling operations
pub type RequestId = Uuid;

/// Slot number on Solana
pub type Slot = u64;

/// Lamports (smallest unit of SOL)
pub type Lamports = u64;

/// Compute units
pub type ComputeUnits = u32;

/// Bundle request containing instructions to be processed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleRequest {
    /// Unique identifier for this request
    pub request_id: RequestId,
    
    /// Whether all transactions must succeed (atomic) or best-effort
    pub atomic: bool,
    
    /// Compute budget configuration
    pub compute: ComputeConfig,
    
    /// Optional Address Lookup Tables to use
    #[serde(default)]
    pub alt_tables: Vec<Pubkey>,
    
    /// Instructions to bundle
    pub instructions: Vec<InstructionData>,
    
    /// Additional signers (beyond fee payer)
    #[serde(default)]
    pub signers: Vec<SignerConfig>,
    
    /// Optional metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

/// Compute budget configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeConfig {
    /// Compute unit limit ("auto" or specific number)
    pub limit: ComputeLimit,
    
    /// Compute unit price ("auto" or specific lamports)
    pub price: ComputePrice,
    
    /// Maximum price willing to pay (lamports per CU)
    #[serde(default = "default_max_price")]
    pub max_price_lamports: Lamports,
}

fn default_max_price() -> Lamports {
    10_000 // 10k lamports per CU max
}

/// Compute unit limit specification
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ComputeLimit {
    Auto,
    Fixed(ComputeUnits),
}

/// Compute unit price specification
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ComputePrice {
    Auto,
    Fixed(Lamports),
}

/// Instruction data with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstructionData {
    /// Program ID to invoke
    pub program_id: Pubkey,
    
    /// Account keys with metadata
    pub keys: Vec<AccountMeta>,
    
    /// Instruction data (base64 encoded)
    pub data_b64: String,
}

/// Account metadata for instructions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountMeta {
    /// Account public key
    pub pubkey: Pubkey,
    
    /// Whether this account must sign the transaction
    pub is_signer: bool,
    
    /// Whether this account is writable
    pub is_writable: bool,
}

/// Signer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignerConfig {
    /// Signer type and location
    #[serde(flatten)]
    pub signer_type: SignerType,
    
    /// Optional alias for this signer
    pub alias: Option<String>,
}

/// Types of signers supported
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SignerType {
    /// File-based keypair (development only)
    File { path: String },
    
    /// AWS KMS key
    Kms { 
        key_id: String,
        region: Option<String>,
    },
    
    /// Environment variable containing base58 private key
    Env { var_name: String },
}

/// Response from bundle submission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleResponse {
    /// Original request ID
    pub request_id: RequestId,
    
    /// Overall status of the bundle
    pub status: BundleStatus,
    
    /// Individual transaction results
    pub transactions: Vec<TransactionResult>,
    
    /// URL to detailed logs
    pub logs_url: Option<String>,
    
    /// Processing metrics
    pub metrics: BundleMetrics,
    
    /// Timestamp when processing completed
    pub completed_at: DateTime<Utc>,
}

/// Overall bundle status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BundleStatus {
    /// All transactions succeeded
    Success,
    
    /// Some transactions succeeded (non-atomic mode)
    Partial,
    
    /// All transactions failed
    Failed,
    
    /// Still processing
    Processing,
}

/// Result of a single transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionResult {
    /// Transaction signature
    pub signature: Signature,
    
    /// Slot where transaction was included
    pub slot: Option<Slot>,
    
    /// Current confirmation status
    pub status: TransactionStatus,
    
    /// Compute units consumed
    pub compute_units_consumed: Option<ComputeUnits>,
    
    /// Fee paid in lamports
    pub fee_paid_lamports: Option<Lamports>,
    
    /// Transaction logs
    #[serde(default)]
    pub logs: Vec<String>,
    
    /// Error information if failed
    pub error: Option<TransactionError>,
}

/// Transaction confirmation status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TransactionStatus {
    /// Transaction submitted to network
    Submitted,
    
    /// Transaction processed by a validator
    Processed,
    
    /// Transaction confirmed by cluster
    Confirmed,
    
    /// Transaction finalized (irreversible)
    Finalized,
    
    /// Transaction failed
    Failed,
    
    /// Transaction timed out
    Timeout,
}

/// Transaction error information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionError {
    /// Error type classification
    pub error_type: ErrorType,
    
    /// Human-readable error message
    pub message: String,
    
    /// Raw error details from Solana
    pub raw_error: Option<String>,
    
    /// Whether this error is retryable
    pub retryable: bool,
}

/// Classification of transaction errors
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorType {
    /// Insufficient funds for transaction
    InsufficientFunds,
    
    /// Account is locked by another transaction
    AccountInUse,
    
    /// Blockhash has expired
    BlockhashNotFound,
    
    /// Compute budget exceeded
    ComputeBudgetExceeded,
    
    /// Program execution failed
    ProgramError,
    
    /// Network or RPC error
    NetworkError,
    
    /// Invalid transaction format
    InvalidTransaction,
    
    /// Transaction simulation failed
    Simulation,
    
    /// Unknown or unclassified error
    Unknown,
}

/// Bundle execution metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleMetrics {
    /// Total number of transactions in bundle
    pub total_transactions: u32,
    
    /// Number of successful transactions
    pub successful_transactions: u32,
    
    /// Number of failed transactions
    pub failed_transactions: u32,
    
    /// Total compute units consumed
    pub total_compute_units: u64,
    
    /// Total fee paid in lamports
    pub total_fee_paid: u64,
    
    /// Total processing time in milliseconds
    pub processing_time_ms: u64,
    
    /// Total end-to-end latency
    pub total_latency_ms: u64,
    
    /// Time spent on simulation
    pub simulation_time_ms: u64,
    
    /// Time spent on signing
    pub signing_time_ms: u64,
    
    /// Time spent on submission
    pub submission_time_ms: u64,
    
    /// Time spent waiting for confirmation
    pub confirmation_time_ms: u64,
    
    /// Number of retry attempts
    pub retry_attempts: u32,
    
    /// RPC endpoints used
    pub rpc_endpoints_used: Vec<String>,
}

/// Configuration for fee strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeStrategy {
    /// Base fee in lamports
    pub base_fee_lamports: u64,
    
    /// Priority fee in lamports
    pub priority_fee_lamports: u64,
    
    /// Compute unit price in micro-lamports
    pub compute_unit_price_micro_lamports: u64,
    
    /// Maximum price in lamports per transaction
    pub max_price_lamports: u64,
    
    /// Base strategy (P75 of recent fees)
    pub base_percentile: u8,
    
    /// Additional buffer percentage
    pub buffer_percent: u8,
    
    /// Enable adaptive fee adjustment
    pub adaptive: bool,
    
    /// Enable fee bumping on timeout
    pub enable_bump: bool,
    
    /// Maximum fee bump attempts
    pub max_bump_attempts: u32,
}

impl Default for FeeStrategy {
    fn default() -> Self {
        Self {
            base_fee_lamports: 5000,
            priority_fee_lamports: 0,
            compute_unit_price_micro_lamports: 1,
            max_price_lamports: 100_000_000, // 0.1 SOL max
            base_percentile: 75,
            buffer_percent: 10,
            adaptive: true,
            enable_bump: true,
            max_bump_attempts: 3,
        }
    }
}

/// RPC endpoint configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcEndpoint {
    /// RPC URL
    pub url: String,
    
    /// Weight for load balancing (higher = more preferred)
    pub weight: u32,
    
    /// Whether this endpoint supports Jito bundles
    pub supports_jito: bool,
    
    /// Optional authentication token
    pub auth_token: Option<String>,
}

/// Jito bundle configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JitoConfig {
    /// Jito relayer URL
    pub relayer_url: String,
    
    /// Authentication token
    pub auth_token: Option<String>,
    
    /// Tip amount in lamports
    pub tip_lamports: Lamports,
    
    /// Tip account (if not default)
    pub tip_account: Option<Pubkey>,
}

/// Health check status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    /// Overall health
    pub healthy: bool,
    
    /// Individual component status
    pub components: HashMap<String, ComponentHealth>,
    
    /// Timestamp of health check
    pub timestamp: DateTime<Utc>,
}

/// Health status of individual components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    /// Component is healthy
    pub healthy: bool,
    
    /// Optional status message
    pub message: Option<String>,
    
    /// Last successful operation timestamp
    pub last_success: Option<DateTime<Utc>>,
}

/// Bundler-specific errors
#[derive(Error, Debug)]
pub enum BundlerError {
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("RPC error: {0}")]
    Rpc(String),
    
    #[error("Signing error: {0}")]
    Signing(String),
    
    #[error("Simulation failed: {0}")]
    Simulation(String),
    
    #[error("Transaction error: {0}")]
    Transaction(String),
    
    #[error("Timeout: {0}")]
    Timeout(String),
    
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    
    #[error("Internal error: {0}")]
    Internal(String),
}

pub type BundlerResult<T> = Result<T, BundlerError>;

// Utility functions for type conversions
impl From<InstructionData> for Instruction {
    fn from(data: InstructionData) -> Self {
        let data_bytes = general_purpose::STANDARD
            .decode(&data.data_b64)
            .unwrap_or_default();
        
        let accounts = data.keys.into_iter()
            .map(|meta| solana_sdk::instruction::AccountMeta {
                pubkey: meta.pubkey,
                is_signer: meta.is_signer,
                is_writable: meta.is_writable,
            })
            .collect();
        
        Instruction {
            program_id: data.program_id,
            accounts,
            data: data_bytes,
        }
    }
}

impl From<solana_sdk::instruction::AccountMeta> for AccountMeta {
    fn from(meta: solana_sdk::instruction::AccountMeta) -> Self {
        Self {
            pubkey: meta.pubkey,
            is_signer: meta.is_signer,
            is_writable: meta.is_writable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_bundle_request_serialization() {
        let request = BundleRequest {
            request_id: Uuid::new_v4(),
            atomic: true,
            compute: ComputeConfig {
                limit: ComputeLimit::Auto,
                price: ComputePrice::Auto,
                max_price_lamports: 5000,
            },
            alt_tables: vec![],
            instructions: vec![],
            signers: vec![],
            metadata: HashMap::new(),
        };
        
        let json = serde_json::to_string(&request).unwrap();
        let deserialized: BundleRequest = serde_json::from_str(&json).unwrap();
        
        assert_eq!(request.request_id, deserialized.request_id);
        assert_eq!(request.atomic, deserialized.atomic);
    }
    
    #[test]
    fn test_compute_config_variants() {
        let auto_config = ComputeConfig {
            limit: ComputeLimit::Auto,
            price: ComputePrice::Auto,
            max_price_lamports: 1000,
        };
        
        let json = serde_json::to_string(&auto_config).unwrap();
        let _: ComputeConfig = serde_json::from_str(&json).unwrap();
        
        let fixed_config = ComputeConfig {
            limit: ComputeLimit::Fixed(200_000),
            price: ComputePrice::Fixed(1000),
            max_price_lamports: 5000,
        };
        
        let json = serde_json::to_string(&fixed_config).unwrap();
        let _: ComputeConfig = serde_json::from_str(&json).unwrap();
    }
}

#[cfg(test)]
mod tests;
