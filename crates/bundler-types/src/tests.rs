//! Comprehensive tests for bundler-types
//! 
//! This module provides extensive test coverage for all types and functionality
//! to ensure the core data structures work correctly before compilation.

use super::*;
use serde_json;
use uuid::Uuid;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bundle_request_serialization() {
        let request = BundleRequest {
            request_id: Uuid::new_v4(),
            atomic: true,
            compute: ComputeConfig {
                units: 200000,
                unit_price: 1000,
            },
            alt_tables: vec![],
            instructions: vec![
                InstructionData {
                    program_id: solana_sdk::system_program::id(),
                    accounts: vec![
                        AccountMeta {
                            pubkey: Pubkey::new_unique(),
                            is_signer: true,
                            is_writable: true,
                        }
                    ],
                    data: vec![1, 2, 3, 4],
                }
            ],
            signers: vec![],
            metadata: HashMap::new(),
        };

        // Test serialization
        let json = serde_json::to_string(&request).expect("Should serialize");
        println!("Serialized: {}", json);

        // Test deserialization
        let deserialized: BundleRequest = serde_json::from_str(&json).expect("Should deserialize");
        assert_eq!(request.request_id, deserialized.request_id);
        assert_eq!(request.atomic, deserialized.atomic);
        assert_eq!(request.compute.units, deserialized.compute.units);
    }

    #[test]
    fn test_bundle_response_serialization() {
        let response = BundleResponse {
            request_id: Uuid::new_v4(),
            status: BundleStatus::Success,
            transactions: vec![
                TransactionResult {
                    signature: Signature::new_unique(),
                    status: TransactionStatus::Confirmed,
                    slot: Some(12345),
                    compute_units_consumed: Some(150000),
                    fee_paid_lamports: Some(5000),
                    logs: vec!["Program log: Success".to_string()],
                    error: None,
                }
            ],
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
                submission_time_ms: 300,
                confirmation_time_ms: 900,
                retry_attempts: 0,
                rpc_endpoints_used: vec!["https://api.mainnet-beta.solana.com".to_string()],
            },
            completed_at: chrono::Utc::now(),
        };

        // Test serialization
        let json = serde_json::to_string(&response).expect("Should serialize");
        println!("Response serialized: {}", json);

        // Test deserialization
        let deserialized: BundleResponse = serde_json::from_str(&json).expect("Should deserialize");
        assert_eq!(response.request_id, deserialized.request_id);
        assert_eq!(response.status, deserialized.status);
        assert_eq!(response.metrics.total_transactions, deserialized.metrics.total_transactions);
    }

    #[test]
    fn test_bundle_status_equality() {
        assert_eq!(BundleStatus::Success, BundleStatus::Success);
        assert_ne!(BundleStatus::Success, BundleStatus::Failed);
        assert_eq!(BundleStatus::Partial, BundleStatus::Partial);
    }

    #[test]
    fn test_transaction_status_equality() {
        assert_eq!(TransactionStatus::Confirmed, TransactionStatus::Confirmed);
        assert_ne!(TransactionStatus::Confirmed, TransactionStatus::Failed);
    }

    #[test]
    fn test_error_types() {
        let error = BundlerError::InvalidInstruction("test error".to_string());
        assert!(matches!(error, BundlerError::InvalidInstruction(_)));

        let rpc_error = BundlerError::RpcError("connection failed".to_string());
        assert!(matches!(rpc_error, BundlerError::RpcError(_)));
    }

    #[test]
    fn test_instruction_data() {
        let instruction = InstructionData {
            program_id: solana_sdk::system_program::id(),
            accounts: vec![
                AccountMeta {
                    pubkey: Pubkey::new_unique(),
                    is_signer: true,
                    is_writable: false,
                }
            ],
            data: vec![0, 1, 2, 3, 4, 5],
        };

        // Test that we can convert to Solana Instruction
        let solana_instruction = Instruction {
            program_id: instruction.program_id,
            accounts: instruction.accounts.iter().map(|meta| solana_sdk::instruction::AccountMeta {
                pubkey: meta.pubkey,
                is_signer: meta.is_signer,
                is_writable: meta.is_writable,
            }).collect(),
            data: instruction.data.clone(),
        };

        assert_eq!(instruction.program_id, solana_instruction.program_id);
        assert_eq!(instruction.data, solana_instruction.data);
    }

    #[test]
    fn test_compute_config() {
        let config = ComputeConfig {
            units: 400000,
            unit_price: 2000,
        };

        // Test serialization
        let json = serde_json::to_string(&config).expect("Should serialize");
        let deserialized: ComputeConfig = serde_json::from_str(&json).expect("Should deserialize");
        
        assert_eq!(config.units, deserialized.units);
        assert_eq!(config.unit_price, deserialized.unit_price);
    }

    #[test]
    fn test_fee_strategy() {
        let strategy = FeeStrategy {
            base_fee_lamports: 5000,
            priority_fee_lamports: 1000,
            compute_unit_price_micro_lamports: 1500,
            max_price_lamports: 100000,
        };

        // Test that max_price is enforced
        assert!(strategy.max_price_lamports > strategy.base_fee_lamports);
        assert!(strategy.max_price_lamports > strategy.priority_fee_lamports);
    }

    #[test]
    fn test_bundle_metrics() {
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
            submission_time_ms: 600,
            confirmation_time_ms: 1300,
            retry_attempts: 1,
            rpc_endpoints_used: vec!["https://api.mainnet-beta.solana.com".to_string()],
        };

        assert_eq!(metrics.total_transactions, 5);
        assert_eq!(metrics.successful_transactions + metrics.failed_transactions, metrics.total_transactions);
        assert!(metrics.processing_time_ms > 0);
        assert!(metrics.total_latency_ms >= metrics.processing_time_ms);
    }

    #[test]
    fn test_health_status() {
        let mut components = HashMap::new();
        components.insert("rpc".to_string(), ComponentHealth {
            healthy: true,
            message: "OK".to_string(),
            last_success: Some(chrono::Utc::now()),
            error_count: 0,
        });

        let health = HealthStatus {
            healthy: true,
            message: "All systems operational".to_string(),
            last_check: chrono::Utc::now(),
            components,
        };

        assert!(health.healthy);
        assert!(!health.message.is_empty());
        assert!(health.components.contains_key("rpc"));
    }

    #[test]
    fn test_component_health() {
        let component = ComponentHealth {
            healthy: false,
            message: "Connection timeout".to_string(),
            last_success: Some(chrono::Utc::now()),
            error_count: 3,
        };

        assert!(!component.healthy);
        assert_eq!(component.error_count, 3);
        assert!(component.last_success.is_some());
    }

    #[test]
    fn test_rpc_endpoint_health() {
        let endpoint = RpcEndpointHealth {
            url: "https://api.mainnet-beta.solana.com".to_string(),
            healthy: true,
            latency_ms: 150,
            last_success: chrono::Utc::now(),
            error_count: 0,
            success_rate: 0.99,
        };

        assert!(endpoint.healthy);
        assert_eq!(endpoint.error_count, 0);
        assert!(endpoint.success_rate > 0.9);
        assert!(endpoint.latency_ms > 0);
    }

    #[test]
    fn test_signer_config() {
        let signer = SignerConfig {
            signer_type: SignerType::File,
            path: Some("/path/to/keypair.json".to_string()),
            env_var: None,
            kms_key_id: None,
        };

        assert!(matches!(signer.signer_type, SignerType::File));
        assert!(signer.path.is_some());
        assert!(signer.env_var.is_none());
    }

    #[test]
    fn test_json_compatibility() {
        // Test that our types work with standard JSON libraries
        let request = BundleRequest {
            request_id: Uuid::new_v4(),
            atomic: false,
            compute: ComputeConfig {
                units: 200000,
                unit_price: 1000,
            },
            alt_tables: vec![],
            instructions: vec![],
            signers: vec![],
            metadata: HashMap::new(),
        };

        let json_value: serde_json::Value = serde_json::to_value(&request).expect("Should convert to JSON");
        assert!(json_value.is_object());
        
        let back: BundleRequest = serde_json::from_value(json_value).expect("Should convert back");
        assert_eq!(request.request_id, back.request_id);
        assert_eq!(request.atomic, back.atomic);
    }

    #[test]
    fn test_error_display() {
        let error = BundlerError::InvalidInstruction("test message".to_string());
        let display_str = format!("{}", error);
        assert!(display_str.contains("test message"));
    }

    #[test]
    fn test_uuid_generation() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        assert_ne!(id1, id2);
        
        // Test that UUIDs serialize properly
        let json = serde_json::to_string(&id1).expect("Should serialize UUID");
        let parsed: Uuid = serde_json::from_str(&json).expect("Should parse UUID");
        assert_eq!(id1, parsed);
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_full_bundle_workflow() {
        // Create a complete bundle request
        let request = BundleRequest {
            request_id: Uuid::new_v4(),
            atomic: true,
            compute: ComputeConfig {
                units: 400000,
                unit_price: 2000,
            },
            alt_tables: vec![],
            instructions: vec![
                InstructionData {
                    program_id: solana_sdk::system_program::id(),
                    accounts: vec![
                        AccountMeta {
                            pubkey: Pubkey::new_unique(),
                            is_signer: true,
                            is_writable: true,
                        },
                        AccountMeta {
                            pubkey: Pubkey::new_unique(),
                            is_signer: false,
                            is_writable: true,
                        },
                    ],
                    data: vec![2, 0, 0, 0, 100, 0, 0, 0, 0, 0, 0, 0], // Transfer instruction
                }
            ],
            signers: vec![],
            metadata: {
                let mut meta = HashMap::new();
                meta.insert("client".to_string(), "test-client".to_string());
                meta
            },
        };

        // Serialize to JSON
        let json = serde_json::to_string_pretty(&request).expect("Should serialize");
        println!("Full bundle request JSON:\n{}", json);

        // Deserialize back
        let parsed: BundleRequest = serde_json::from_str(&json).expect("Should parse");
        assert_eq!(request.request_id, parsed.request_id);
        assert_eq!(request.instructions.len(), parsed.instructions.len());
        assert_eq!(request.atomic, parsed.atomic);

        // Create response
        let response = BundleResponse {
            request_id: request.request_id,
            status: BundleStatus::Success,
            transactions: vec![
                TransactionResult {
                    signature: Signature::new_unique(),
                    status: TransactionStatus::Confirmed,
                    slot: Some(12345678),
                    compute_units_consumed: Some(350000),
                    fee_paid_lamports: Some(8000),
                    logs: vec![
                        "Program 11111111111111111111111111111112 invoke [1]".to_string(),
                        "Program 11111111111111111111111111111112 success".to_string(),
                    ],
                    error: None,
                }
            ],
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
                submission_time_ms: 200,
                confirmation_time_ms: 800,
                retry_attempts: 0,
                rpc_endpoints_used: vec!["https://api.mainnet-beta.solana.com".to_string()],
            },
            completed_at: chrono::Utc::now(),
        };

        // Verify response serialization
        let response_json = serde_json::to_string_pretty(&response).expect("Should serialize response");
        println!("Full bundle response JSON:\n{}", response_json);

        let parsed_response: BundleResponse = serde_json::from_str(&response_json).expect("Should parse response");
        assert_eq!(response.request_id, parsed_response.request_id);
        assert_eq!(response.status, parsed_response.status);
        assert_eq!(response.metrics.total_transactions, parsed_response.metrics.total_transactions);
    }

    #[test]
    fn test_error_scenarios() {
        // Test failed bundle
        let failed_response = BundleResponse {
            request_id: Uuid::new_v4(),
            status: BundleStatus::Failed,
            transactions: vec![
                TransactionResult {
                    signature: Signature::new_unique(),
                    status: TransactionStatus::Failed,
                    slot: Some(12345),
                    compute_units_consumed: None,
                    fee_paid_lamports: Some(5000), // Fee still paid even on failure
                    logs: vec![
                        "Program log: Error: insufficient funds".to_string(),
                    ],
                    error: Some(TransactionError {
                        error_type: ErrorType::InsufficientFunds,
                        message: "Insufficient funds for transaction".to_string(),
                        raw_error: Some("InsufficientFundsForRent".to_string()),
                        retryable: false,
                    }),
                }
            ],
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
                submission_time_ms: 150,
                confirmation_time_ms: 500,
                retry_attempts: 2,
                rpc_endpoints_used: vec!["https://api.mainnet-beta.solana.com".to_string()],
            },
            completed_at: chrono::Utc::now(),
        };

        assert_eq!(failed_response.status, BundleStatus::Failed);
        assert_eq!(failed_response.metrics.successful_transactions, 0);
        assert_eq!(failed_response.metrics.failed_transactions, 1);
    }
}
