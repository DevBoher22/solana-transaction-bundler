use bundler_config::BundlerConfigBuilder;
use bundler_core::BundlerService;
use bundler_types::*;
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use solana_sdk::{
    instruction::Instruction,
    pubkey::Pubkey,
    system_instruction,
    signature::Keypair,
    signer::Signer,
};
use std::collections::HashMap;
use tokio::runtime::Runtime;
use uuid::Uuid;

/// Create a test bundler service for benchmarking
async fn create_benchmark_service() -> BundlerService {
    let config = BundlerConfigBuilder::new()
        .with_rpc_endpoint("https://api.devnet.solana.com".to_string(), 100)
        .build()
        .expect("Failed to build benchmark config");
    
    BundlerService::new(config)
        .await
        .expect("Failed to create bundler service")
}

/// Create a bundle request with specified number of instructions
fn create_bundle_request(num_instructions: usize) -> BundleRequest {
    let mut instructions = Vec::new();
    
    for _ in 0..num_instructions {
        let from = Keypair::new();
        let to = Keypair::new();
        let instruction = system_instruction::transfer(&from.pubkey(), &to.pubkey(), 1000);
        
        let instruction_data = InstructionData {
            program_id: instruction.program_id,
            keys: instruction.accounts.into_iter().map(|meta| AccountMeta {
                pubkey: meta.pubkey,
                is_signer: meta.is_signer,
                is_writable: meta.is_writable,
            }).collect(),
            data_b64: base64::engine::general_purpose::STANDARD.encode(&instruction.data),
        };
        
        instructions.push(instruction_data);
    }
    
    BundleRequest {
        request_id: Uuid::new_v4(),
        atomic: true,
        compute: ComputeConfig {
            limit: ComputeLimit::Auto,
            price: ComputePrice::Auto,
            max_price_lamports: 10_000,
        },
        alt_tables: vec![],
        instructions,
        signers: vec![],
        metadata: HashMap::new(),
    }
}

/// Benchmark bundle request serialization/deserialization
fn bench_bundle_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("bundle_serialization");
    
    for size in [1, 5, 10, 20, 50].iter() {
        let bundle_request = create_bundle_request(*size);
        
        group.bench_with_input(
            BenchmarkId::new("serialize", size),
            size,
            |b, _| {
                b.iter(|| {
                    let json = serde_json::to_string(black_box(&bundle_request)).unwrap();
                    black_box(json);
                });
            },
        );
        
        let json = serde_json::to_string(&bundle_request).unwrap();
        group.bench_with_input(
            BenchmarkId::new("deserialize", size),
            size,
            |b, _| {
                b.iter(|| {
                    let request: BundleRequest = serde_json::from_str(black_box(&json)).unwrap();
                    black_box(request);
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark instruction conversion
fn bench_instruction_conversion(c: &mut Criterion) {
    let mut group = c.benchmark_group("instruction_conversion");
    
    for size in [1, 10, 50, 100].iter() {
        let bundle_request = create_bundle_request(*size);
        
        group.bench_with_input(
            BenchmarkId::new("to_solana_instructions", size),
            size,
            |b, _| {
                b.iter(|| {
                    let instructions: Vec<Instruction> = bundle_request.instructions
                        .iter()
                        .map(|ix| black_box(ix.clone().into()))
                        .collect();
                    black_box(instructions);
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark fee calculation (mock)
fn bench_fee_calculation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let service = rt.block_on(create_benchmark_service());
    
    let mut group = c.benchmark_group("fee_calculation");
    
    for num_accounts in [1, 5, 10, 20, 50].iter() {
        let accounts: Vec<Pubkey> = (0..*num_accounts)
            .map(|_| Pubkey::new_unique())
            .collect();
        
        group.bench_with_input(
            BenchmarkId::new("calculate_fee", num_accounts),
            num_accounts,
            |b, _| {
                b.to_async(&rt).iter(|| async {
                    // Note: This will likely fail in benchmark environment
                    // but we're measuring the overhead of the call structure
                    let result = service.fee_manager.calculate_fee(black_box(&accounts)).await;
                    black_box(result);
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark health check operations
fn bench_health_check(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let service = rt.block_on(create_benchmark_service());
    
    c.bench_function("health_check", |b| {
        b.to_async(&rt).iter(|| async {
            let health = service.health_check().await;
            black_box(health);
        });
    });
}

/// Benchmark instruction validation
fn bench_instruction_validation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let service = rt.block_on(create_benchmark_service());
    
    let mut group = c.benchmark_group("instruction_validation");
    
    for size in [1, 10, 50, 100].iter() {
        let bundle_request = create_bundle_request(*size);
        let instructions: Vec<Instruction> = bundle_request.instructions
            .iter()
            .map(|ix| ix.clone().into())
            .collect();
        
        group.bench_with_input(
            BenchmarkId::new("validate_instructions", size),
            size,
            |b, _| {
                b.iter(|| {
                    let result = service.simulator.validate_instructions(black_box(&instructions));
                    black_box(result);
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark transaction creation
fn bench_transaction_creation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let service = rt.block_on(create_benchmark_service());
    
    let mut group = c.benchmark_group("transaction_creation");
    
    for size in [1, 5, 10, 20].iter() {
        let bundle_request = create_bundle_request(*size);
        let instructions: Vec<Instruction> = bundle_request.instructions
            .iter()
            .map(|ix| ix.clone().into())
            .collect();
        
        group.bench_with_input(
            BenchmarkId::new("create_transaction", size),
            size,
            |b, _| {
                b.to_async(&rt).iter(|| async {
                    // Get fee payer (might fail in benchmark environment)
                    if let Ok(fee_payer) = service.signing_manager.fee_payer_pubkey().await {
                        let transaction = solana_sdk::transaction::Transaction::new_with_payer(
                            black_box(&instructions),
                            Some(&fee_payer),
                        );
                        black_box(transaction);
                    }
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark compute unit estimation logic
fn bench_compute_estimation(c: &mut Criterion) {
    c.bench_function("compute_unit_buffer_calculation", |b| {
        b.iter(|| {
            let consumed = black_box(50_000u32);
            let buffered = (consumed as f64 * 1.2) as u32;
            let final_estimate = buffered.max(1_000).min(1_400_000);
            black_box(final_estimate);
        });
    });
}

/// Benchmark error handling overhead
fn bench_error_handling(c: &mut Criterion) {
    c.bench_function("error_creation", |b| {
        b.iter(|| {
            let error = BundlerError::InvalidInput(black_box("Test error".to_string()));
            black_box(error);
        });
    });
    
    c.bench_function("error_conversion", |b| {
        b.iter(|| {
            let error = BundlerError::InvalidInput("Test error".to_string());
            let error_string = error.to_string();
            black_box(error_string);
        });
    });
}

/// Benchmark concurrent operations
fn bench_concurrent_health_checks(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let service = std::sync::Arc::new(rt.block_on(create_benchmark_service()));
    
    c.bench_function("concurrent_health_checks", |b| {
        b.to_async(&rt).iter(|| async {
            let mut handles = vec![];
            
            for _ in 0..10 {
                let service_clone = std::sync::Arc::clone(&service);
                let handle = tokio::spawn(async move {
                    service_clone.health_check().await
                });
                handles.push(handle);
            }
            
            // Wait for all to complete
            for handle in handles {
                let _ = handle.await;
            }
        });
    });
}

/// Benchmark memory allocation patterns
fn bench_memory_allocation(c: &mut Criterion) {
    c.bench_function("bundle_request_allocation", |b| {
        b.iter(|| {
            let request = create_bundle_request(black_box(10));
            black_box(request);
        });
    });
    
    c.bench_function("large_bundle_allocation", |b| {
        b.iter(|| {
            let request = create_bundle_request(black_box(100));
            black_box(request);
        });
    });
}

/// Benchmark configuration operations
fn bench_configuration(c: &mut Criterion) {
    c.bench_function("config_creation", |b| {
        b.iter(|| {
            let config = BundlerConfigBuilder::new()
                .with_rpc_endpoint("https://api.devnet.solana.com".to_string(), 100)
                .build();
            black_box(config);
        });
    });
    
    let config = BundlerConfigBuilder::new()
        .with_rpc_endpoint("https://api.devnet.solana.com".to_string(), 100)
        .build()
        .unwrap();
    
    c.bench_function("config_validation", |b| {
        b.iter(|| {
            let result = black_box(&config).validate();
            black_box(result);
        });
    });
}

/// Benchmark UUID generation (used for request IDs)
fn bench_uuid_generation(c: &mut Criterion) {
    c.bench_function("uuid_generation", |b| {
        b.iter(|| {
            let uuid = Uuid::new_v4();
            black_box(uuid);
        });
    });
}

/// Benchmark base64 encoding/decoding
fn bench_base64_operations(c: &mut Criterion) {
    let test_data = vec![1u8; 1024]; // 1KB of test data
    let encoded = base64::engine::general_purpose::STANDARD.encode(&test_data);
    
    c.bench_function("base64_encode", |b| {
        b.iter(|| {
            let encoded = base64::engine::general_purpose::STANDARD.encode(black_box(&test_data));
            black_box(encoded);
        });
    });
    
    c.bench_function("base64_decode", |b| {
        b.iter(|| {
            let decoded = base64::engine::general_purpose::STANDARD.decode(black_box(&encoded));
            black_box(decoded);
        });
    });
}

criterion_group!(
    benches,
    bench_bundle_serialization,
    bench_instruction_conversion,
    bench_fee_calculation,
    bench_health_check,
    bench_instruction_validation,
    bench_transaction_creation,
    bench_compute_estimation,
    bench_error_handling,
    bench_concurrent_health_checks,
    bench_memory_allocation,
    bench_configuration,
    bench_uuid_generation,
    bench_base64_operations
);

criterion_main!(benches);
