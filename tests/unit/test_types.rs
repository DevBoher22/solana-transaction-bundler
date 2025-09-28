use bundler_types::*;
use chrono::Utc;
use serde_json;
use solana_sdk::{pubkey::Pubkey, signature::Signature};
use std::collections::HashMap;
use uuid::Uuid;

#[test]
fn test_bundle_request_serialization() {
    let request = BundleRequest {
        request_id: Uuid::new_v4(),
        atomic: true,
        compute: ComputeConfig {
            limit: ComputeLimit::Fixed(200_000),
            price: ComputePrice::Auto,
            max_price_lamports: 10_000,
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
                data_b64: "dGVzdA==".to_string(), // "test" in base64
            }
        ],
        signers: vec![],
        metadata: HashMap::new(),
    };
    
    // Test serialization
    let json = serde_json::to_string(&request).unwrap();
    assert!(json.contains("request_id"));
    assert!(json.contains("atomic"));
    assert!(json.contains("instructions"));
    
    // Test deserialization
    let deserialized: BundleRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.request_id, request.request_id);
    assert_eq!(deserialized.atomic, request.atomic);
    assert_eq!(deserialized.instructions.len(), 1);
}

#[test]
fn test_bundle_response_creation() {
    let request_id = Uuid::new_v4();
    let signature = Signature::new_unique();
    let failed_signature = Signature::new_unique();
    let bundle_signature = Signature::new_unique();

    let response = BundleResponse {
        request_id,
        status: BundleStatus::Success,
        transactions: vec![
            TransactionResult {
                signature: Some(signature),
                status: TransactionStatus::Finalized,
                compute_units_consumed: Some(50_000),
                fee_paid_lamports: Some(5_000),
                logs: vec!["Program log: Success".to_string()],
                error: None,
            },
            TransactionResult {
                signature: Some(failed_signature),
                status: TransactionStatus::Failed,
                compute_units_consumed: None,
                fee_paid_lamports: None,
                logs: vec!["Program log: Failure".to_string()],
                error: Some("Transaction failed".to_string()),
            },
        ],
        bundle_signature: Some(bundle_signature),
        slot: Some(12345),
        blockhash: Some("ExampleBlockhash".to_string()),
        confirmation: ConfirmationStatus::Finalized,
        logs_url: Some(format!("/logs/{}", request_id)),
        metrics: BundleMetrics {
            total_transactions: 2,
            successful_transactions: 1,
            failed_transactions: 1,
            total_compute_units: 50_000,
            total_fee_paid: 5_000,
            processing_time_ms: 1_200,
            total_latency_ms: 1500,
            simulation_time_ms: 200,
            signing_time_ms: 50,
            submission_time_ms: 800,
            confirmation_time_ms: 450,
            retry_attempts: 0,
            rpc_endpoints_used: vec!["https://api.mainnet-beta.solana.com".to_string()],
        },
        completed_at: Utc::now(),
    };

    assert_eq!(response.status, BundleStatus::Success);
    assert_eq!(response.transactions.len(), 2);
    assert!(response.metrics.total_latency_ms > 0);
    assert_eq!(response.metrics.total_transactions, response.transactions.len() as u32);
    assert_eq!(response.bundle_signature, Some(bundle_signature));
    assert_eq!(response.confirmation, ConfirmationStatus::Finalized);
}

#[test]
fn test_compute_config_variants() {
    // Test Auto limit
    let auto_config = ComputeConfig {
        limit: ComputeLimit::Auto,
        price: ComputePrice::Auto,
        max_price_lamports: 50_000,
    };
    
    let json = serde_json::to_string(&auto_config).unwrap();
    let deserialized: ComputeConfig = serde_json::from_str(&json).unwrap();
    
    match deserialized.limit {
        ComputeLimit::Auto => assert!(true),
        _ => panic!("Expected Auto limit"),
    }
    
    // Test Fixed limit
    let fixed_config = ComputeConfig {
        limit: ComputeLimit::Fixed(300_000),
        price: ComputePrice::Fixed(2_000),
        max_price_lamports: 50_000,
    };
    
    let json = serde_json::to_string(&fixed_config).unwrap();
    let deserialized: ComputeConfig = serde_json::from_str(&json).unwrap();
    
    match deserialized.limit {
        ComputeLimit::Fixed(units) => assert_eq!(units, 300_000),
        _ => panic!("Expected Fixed limit"),
    }
    
    match deserialized.price {
        ComputePrice::Fixed(price) => assert_eq!(price, 2_000),
        _ => panic!("Expected Fixed price"),
    }
}

#[test]
fn test_transaction_error_types() {
    let result = TransactionResult {
        signature: Some(Signature::new_unique()),
        status: TransactionStatus::Failed,
        compute_units_consumed: None,
        fee_paid_lamports: None,
        logs: vec!["Program log: failure".to_string()],
        error: Some("Account is locked".to_string()),
    };

    let json = serde_json::to_string(&result).unwrap();
    let deserialized: TransactionResult = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.status, TransactionStatus::Failed);
    assert_eq!(deserialized.error.as_deref(), Some("Account is locked"));
}

#[test]
fn test_health_status_components() {
    let mut components = HashMap::new();
    components.insert("rpc".to_string(), ComponentHealth {
        healthy: true,
        message: Some("All endpoints responding".to_string()),
        last_success: Some(Utc::now()),
    });
    components.insert("signing".to_string(), ComponentHealth {
        healthy: false,
        message: Some("KMS timeout".to_string()),
        last_success: None,
    });

    let health = HealthStatus {
        healthy: false, // Overall unhealthy due to signing component
        timestamp: Utc::now(),
        components,
    };
    
    assert!(!health.healthy);
    assert_eq!(health.components.len(), 2);
    assert!(health.components.get("rpc").unwrap().healthy);
    assert!(!health.components.get("signing").unwrap().healthy);
}

#[test]
fn test_bundle_metrics_calculation() {
    let metrics = BundleMetrics {
        total_transactions: 3,
        successful_transactions: 2,
        failed_transactions: 1,
        total_compute_units: 150_000,
        total_fee_paid: 15_000,
        processing_time_ms: 1_500,
        total_latency_ms: 2000,
        simulation_time_ms: 300,
        signing_time_ms: 100,
        submission_time_ms: 1200,
        confirmation_time_ms: 400,
        retry_attempts: 2,
        rpc_endpoints_used: vec![
            "endpoint1".to_string(),
            "endpoint2".to_string(),
        ],
    };

    // Verify that component times don't exceed total
    let component_sum = metrics.simulation_time_ms
        + metrics.signing_time_ms
        + metrics.submission_time_ms
        + metrics.confirmation_time_ms;

    assert!(component_sum <= metrics.total_latency_ms);
    assert_eq!(metrics.rpc_endpoints_used.len(), 2);
    assert_eq!(metrics.retry_attempts, 2);
    assert_eq!(metrics.total_transactions, 3);
    assert_eq!(metrics.successful_transactions + metrics.failed_transactions, metrics.total_transactions);
}

#[test]
fn test_instruction_data_conversion() {
    use base64::{engine::general_purpose, Engine};
    
    let test_data = vec![1, 2, 3, 4, 5];
    let encoded = general_purpose::STANDARD.encode(&test_data);
    
    let instruction_data = InstructionData {
        program_id: Pubkey::new_unique(),
        keys: vec![
            AccountMeta {
                pubkey: Pubkey::new_unique(),
                is_signer: false,
                is_writable: true,
            }
        ],
        data_b64: encoded,
    };
    
    // Test conversion to Solana instruction
    let solana_instruction: solana_sdk::instruction::Instruction = instruction_data.into();
    
    assert_eq!(solana_instruction.data, test_data);
    assert_eq!(solana_instruction.accounts.len(), 1);
    assert!(!solana_instruction.accounts[0].is_signer);
    assert!(solana_instruction.accounts[0].is_writable);
}

#[test]
fn test_bundler_error_types() {
    let errors = vec![
        BundlerError::Config("Invalid RPC endpoint".to_string()),
        BundlerError::Rpc("Connection failed".to_string()),
        BundlerError::Signing("Private key not found".to_string()),
        BundlerError::Transaction("Simulation failed".to_string()),
        BundlerError::Simulation("Program error".to_string()),
        BundlerError::InvalidInput("Malformed request".to_string()),
        BundlerError::Timeout("Request timed out".to_string()),
        BundlerError::InvalidInstruction("Bad instruction".to_string()),
        BundlerError::RpcError("RPC error".to_string()),
        BundlerError::Internal("Internal error".to_string()),
    ];
    
    for error in errors {
        let error_string = error.to_string();
        assert!(!error_string.is_empty());
        
        // Test that error implements required traits
        let _: Box<dyn std::error::Error> = Box::new(error);
    }
}

#[test]
fn test_signer_config_variants() {
    let file_signer = SignerConfig {
        alias: Some("test".to_string()),
        signer_type: SignerType::File {
            path: "/path/to/keypair.json".to_string(),
        },
    };

    let env_signer = SignerConfig {
        alias: Some("env_test".to_string()),
        signer_type: SignerType::Env {
            var_name: "PRIVATE_KEY".to_string(),
        },
    };

    let kms_signer = SignerConfig {
        alias: Some("kms_test".to_string()),
        signer_type: SignerType::Kms {
            key_id: "arn:aws:kms:us-east-1:123456789012:key/12345678-1234-1234-1234-123456789012".to_string(),
            region: Some("us-east-1".to_string()),
        },
    };

    // Test serialization/deserialization
    for signer in vec![file_signer, env_signer, kms_signer] {
        let json = serde_json::to_string(&signer).unwrap();
        let deserialized: SignerConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.alias, signer.alias);
    }
}

#[test]
fn test_fee_strategy_serialization() {
    let strategies = vec![
        FeeStrategy::P75PlusBuffer,
        FeeStrategy::Fixed,
        FeeStrategy::Dynamic,
    ];
    
    for strategy in strategies {
        let json = serde_json::to_string(&strategy).unwrap();
        let deserialized: FeeStrategy = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, strategy);
    }
}

#[test]
fn test_bundle_status_transitions() {
    // Test valid status transitions
    let statuses = vec![
        BundleStatus::Processing,
        BundleStatus::Success,
        BundleStatus::Failed,
        BundleStatus::Timeout,
        BundleStatus::Rejected,
    ];

    for status in statuses {
        let json = serde_json::to_string(&status).unwrap();
        let deserialized: BundleStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, status);
    }
}

#[test]
fn test_transaction_status_hierarchy() {
    let statuses = vec![
        TransactionStatus::Pending,
        TransactionStatus::Processed,
        TransactionStatus::Confirmed,
        TransactionStatus::Finalized,
        TransactionStatus::Failed,
    ];

    // Test that all statuses can be serialized and deserialized
    for status in statuses {
        let json = serde_json::to_string(&status).unwrap();
        let deserialized: TransactionStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, status);
    }
}

#[test]
fn test_rpc_endpoint_weight_validation() {
    let endpoint = RpcEndpoint {
        url: "https://api.mainnet-beta.solana.com".to_string(),
        weight: 100,
        supports_jito: true,
        auth_token: Some("token".to_string()),
    };

    assert!(endpoint.weight > 0);
    assert!(endpoint.url.starts_with("https://"));
    
    // Test serialization
    let json = serde_json::to_string(&endpoint).unwrap();
    let deserialized: RpcEndpoint = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.url, endpoint.url);
    assert_eq!(deserialized.weight, endpoint.weight);
}

#[test]
fn test_jito_config_optional_fields() {
    let jito_config = JitoConfig {
        block_engine_url: "https://mainnet.block-engine.jito.wtf".to_string(),
        relayer_url: "https://relayer.jito.wtf".to_string(),
        auth_keypair_path: Some("/path/to/jito.json".to_string()),
        tip_lamports: 10_000,
        max_tip_lamports: 20_000,
        enabled: true,
    };

    assert!(jito_config.enabled);
    assert_eq!(jito_config.tip_lamports, 10_000);
    assert_eq!(jito_config.auth_keypair_path.as_deref(), Some("/path/to/jito.json"));

    let disabled_jito = JitoConfig {
        block_engine_url: "https://mainnet.block-engine.jito.wtf".to_string(),
        relayer_url: "https://relayer.jito.wtf".to_string(),
        auth_keypair_path: None,
        tip_lamports: 0,
        max_tip_lamports: 5_000,
        enabled: false,
    };

    assert!(!disabled_jito.enabled);
    assert!(disabled_jito.auth_keypair_path.is_none());
    assert_eq!(disabled_jito.max_tip_lamports, 5_000);
}
