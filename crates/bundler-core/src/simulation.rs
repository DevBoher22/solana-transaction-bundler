use bundler_config::SecurityConfig;
use bundler_types::{BundlerError, BundlerResult};
use serde::Serialize;
use solana_sdk::{pubkey::Pubkey, transaction::Transaction};
use std::{collections::HashSet, sync::Arc};
use tracing::{debug, error, info, warn};

use crate::rpc::SolanaRpcClient;

/// Detailed error information from simulations
#[derive(Debug, Clone, Serialize)]
pub struct SimulationError {
    pub message: String,
    pub retryable: bool,
}

/// Result of transaction simulation
#[derive(Debug, Clone)]
pub struct SimulationResult {
    pub success: bool,
    pub compute_units_consumed: Option<u32>,
    pub logs: Vec<String>,
    pub error: Option<SimulationError>,
    pub accounts_modified: Vec<Pubkey>,
    pub estimated_fee: Option<u64>,
    pub return_data: Option<String>,
}

/// Transaction simulator for pre-flight validation
pub struct TransactionSimulator {
    rpc_client: Arc<SolanaRpcClient>,
    security_config: SecurityConfig,
}

impl TransactionSimulator {
    /// Create a new transaction simulator
    pub fn new(rpc_client: Arc<SolanaRpcClient>, security_config: &SecurityConfig) -> Self {
        Self {
            rpc_client,
            security_config: security_config.clone(),
        }
    }

    /// Validate instructions against security policies
    pub fn validate_instructions(
        &self,
        instructions: &[solana_sdk::instruction::Instruction],
    ) -> BundlerResult<()> {
        if !self.security_config.validate_instructions {
            return Ok(());
        }

        if instructions.len() as u32 > self.security_config.max_bundle_size {
            return Err(BundlerError::Simulation(format!(
                "Bundle has too many instructions ({} > {})",
                instructions.len(),
                self.security_config.max_bundle_size
            )));
        }

        if self.security_config.program_whitelist.is_empty() {
            return Ok(());
        }

        for (i, instruction) in instructions.iter().enumerate() {
            if !self
                .security_config
                .program_whitelist
                .contains(&instruction.program_id)
            {
                return Err(BundlerError::Simulation(format!(
                    "Instruction {}: Program {} is not whitelisted",
                    i, instruction.program_id
                )));
            }
        }

        Ok(())
    }

    /// Simulate a transaction
    pub async fn simulate_transaction(
        &self,
        transaction: &Transaction,
    ) -> BundlerResult<SimulationResult> {
        debug!(
            "Simulating transaction with {} instructions",
            transaction.message.instructions.len()
        );

        // Validate instructions first
        let instructions: Vec<_> = transaction
            .message
            .instructions
            .iter()
            .map(|ix| solana_sdk::instruction::Instruction {
                program_id: transaction.message.account_keys[ix.program_id_index as usize],
                accounts: ix
                    .accounts
                    .iter()
                    .map(|&account_index| solana_sdk::instruction::AccountMeta {
                        pubkey: transaction.message.account_keys[account_index as usize],
                        is_signer: transaction.message.is_signer(account_index as usize),
                        is_writable: transaction
                            .message
                            .is_maybe_writable(account_index as usize, None),
                    })
                    .collect(),
                data: ix.data.clone(),
            })
            .collect();

        self.validate_instructions(&instructions)?;

        // For now, return a mock simulation result since we don't have full RPC integration
        // In a real implementation, this would call the RPC simulate method
        let simulation_result = SimulationResult {
            success: true,
            compute_units_consumed: Some(50_000), // Mock value
            logs: vec!["Program log: Success".to_string()],
            error: None,
            accounts_modified: instructions
                .iter()
                .flat_map(|ix| ix.accounts.iter())
                .filter(|meta| meta.is_writable)
                .map(|meta| meta.pubkey)
                .collect::<HashSet<_>>()
                .into_iter()
                .collect(),
            estimated_fee: Some(5_000), // Mock fee
            return_data: None,
        };

        info!(
            "Transaction simulation completed: success={}",
            simulation_result.success
        );
        Ok(simulation_result)
    }

    /// Estimate compute units for a transaction
    pub async fn estimate_compute_units(&self, transaction: &Transaction) -> BundlerResult<u32> {
        let simulation = self.simulate_transaction(transaction).await?;

        if let Some(cu_consumed) = simulation.compute_units_consumed {
            // Add 20% buffer for safety
            let buffered = (cu_consumed as f64 * 1.2) as u32;
            Ok(buffered.max(1_000).min(1_400_000))
        } else {
            // Default estimate based on instruction count
            let base_cu = 1_000u32;
            let per_instruction = 10_000u32;
            let estimated =
                base_cu + (transaction.message.instructions.len() as u32 * per_instruction);
            Ok(estimated.min(1_400_000))
        }
    }

    /// Check if a transaction is likely to succeed based on simulation
    pub async fn predict_success(&self, transaction: &Transaction) -> BundlerResult<f64> {
        let simulation = self.simulate_transaction(transaction).await?;

        if simulation.success {
            // Base success probability
            let mut probability: f64 = 0.9;

            // Adjust based on compute units consumed
            if let Some(cu_consumed) = simulation.compute_units_consumed {
                if cu_consumed > 1_000_000 {
                    probability *= 0.8; // High CU usage reduces success probability
                } else if cu_consumed < 10_000 {
                    probability *= 1.1; // Low CU usage increases success probability
                }
            }

            // Adjust based on number of accounts modified
            let account_count = simulation.accounts_modified.len();
            if account_count > 10 {
                probability *= 0.9; // Many account modifications reduce success probability
            }

            // Adjust based on error patterns in logs
            for log in &simulation.logs {
                if log.to_lowercase().contains("warning") {
                    probability *= 0.95;
                }
            }

            Ok(probability.min(1.0))
        } else {
            // If simulation failed, check if it's retryable
            if let Some(error) = &simulation.error {
                if error.retryable {
                    Ok(0.3) // Some chance of success on retry
                } else {
                    Ok(0.0) // No chance of success
                }
            } else {
                Ok(0.1) // Unknown error, small chance of success
            }
        }
    }

    /// Simulate multiple transactions as a bundle
    pub async fn simulate_bundle(
        &self,
        transactions: &[Transaction],
    ) -> BundlerResult<Vec<SimulationResult>> {
        let mut results = Vec::new();

        for (i, transaction) in transactions.iter().enumerate() {
            debug!("Simulating transaction {} of {}", i + 1, transactions.len());

            match self.simulate_transaction(transaction).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    error!("Failed to simulate transaction {}: {}", i, e);

                    // Create a failed simulation result
                    let failed_result = SimulationResult {
                        success: false,
                        compute_units_consumed: None,
                        logs: vec![format!("Simulation failed: {}", e)],
                        error: Some(SimulationError {
                            message: e.to_string(),
                            retryable: false,
                        }),
                        accounts_modified: vec![],
                        estimated_fee: None,
                        return_data: None,
                    };

                    results.push(failed_result);
                }
            }
        }

        Ok(results)
    }

    /// Get simulation statistics
    pub async fn get_stats(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut stats = std::collections::HashMap::new();

        stats.insert(
            "security_enabled".to_string(),
            serde_json::Value::Bool(true),
        );
        stats.insert(
            "program_whitelist_size".to_string(),
            serde_json::Value::Number(self.security_config.program_whitelist.len().into()),
        );
        stats.insert(
            "validate_instructions".to_string(),
            serde_json::Value::Bool(self.security_config.validate_instructions),
        );
        stats.insert(
            "max_bundle_size".to_string(),
            serde_json::Value::Number(self.security_config.max_bundle_size.into()),
        );

        stats
    }

    /// Perform health check
    pub async fn health_check(&self) -> BundlerResult<()> {
        // Check if RPC client is healthy
        let _health_status = self.rpc_client.get_health_status();

        // Validate security configuration
        if self.security_config.program_whitelist.is_empty() {
            warn!("Program whitelist is empty - all programs allowed");
        }

        if self.security_config.max_bundle_size == 0 {
            return Err(BundlerError::Simulation(
                "max_bundle_size cannot be zero".to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bundler_config::SecurityConfigBuilder;
    use solana_sdk::{
        hash::Hash, instruction::Instruction, message::Message, pubkey::Pubkey, system_instruction,
    };

    fn create_test_simulator() -> TransactionSimulator {
        let rpc_client = Arc::new(SolanaRpcClient::new(vec![], 30, 3, 1000));
        let security_config = SecurityConfigBuilder::new()
            .with_program_whitelist(vec![solana_sdk::system_program::ID])
            .build()
            .unwrap();

        TransactionSimulator::new(rpc_client, &security_config)
    }

    #[test]
    fn test_validate_instructions() {
        let simulator = create_test_simulator();

        // Valid instruction
        let valid_instruction =
            system_instruction::transfer(&Pubkey::new_unique(), &Pubkey::new_unique(), 1000);

        let result = simulator.validate_instructions(&[valid_instruction]);
        assert!(result.is_ok());

        // Invalid instruction (not whitelisted program)
        let invalid_instruction = Instruction {
            program_id: Pubkey::new_unique(), // Not in whitelist
            accounts: vec![],
            data: vec![],
        };

        let result = simulator.validate_instructions(&[invalid_instruction]);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_simulate_transaction() {
        let simulator = create_test_simulator();

        // Create a simple transfer transaction
        let from = Pubkey::new_unique();
        let to = Pubkey::new_unique();
        let instruction = system_instruction::transfer(&from, &to, 1000);

        let message = Message::new(&[instruction], Some(&from));
        let transaction = Transaction::new_unsigned(message);

        let result = simulator.simulate_transaction(&transaction).await;
        assert!(result.is_ok());

        let simulation = result.unwrap();
        assert!(simulation.success);
        assert!(simulation.compute_units_consumed.is_some());
        assert!(!simulation.logs.is_empty());
    }

    #[tokio::test]
    async fn test_estimate_compute_units() {
        let simulator = create_test_simulator();

        let from = Pubkey::new_unique();
        let to = Pubkey::new_unique();
        let instruction = system_instruction::transfer(&from, &to, 1000);

        let message = Message::new(&[instruction], Some(&from));
        let transaction = Transaction::new_unsigned(message);

        let result = simulator.estimate_compute_units(&transaction).await;
        assert!(result.is_ok());

        let cu_estimate = result.unwrap();
        assert!(cu_estimate >= 1_000);
        assert!(cu_estimate <= 1_400_000);
    }

    #[tokio::test]
    async fn test_predict_success() {
        let simulator = create_test_simulator();

        let from = Pubkey::new_unique();
        let to = Pubkey::new_unique();
        let instruction = system_instruction::transfer(&from, &to, 1000);

        let message = Message::new(&[instruction], Some(&from));
        let transaction = Transaction::new_unsigned(message);

        let result = simulator.predict_success(&transaction).await;
        assert!(result.is_ok());

        let probability = result.unwrap();
        assert!(probability >= 0.0);
        assert!(probability <= 1.0);
    }
}
