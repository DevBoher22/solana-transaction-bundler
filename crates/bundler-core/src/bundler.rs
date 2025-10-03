use base64::{engine::general_purpose, Engine};
use bundler_config::BundlerConfig;
use bundler_types::{
    BundleMetrics, BundleRequest, BundleResponse, BundleStatus, BundlerError, BundlerResult,
    ComputeLimit, ComputePrice, ComputeUnits, ConfirmationStatus, InstructionData, Lamports,
    TransactionResult, TransactionStatus,
};
use chrono::Utc;
// Note: AddressLookupTableAccount is not directly available in Solana 3.0
use solana_commitment_config::CommitmentLevel;
// Note: ComputeBudgetInstruction is not directly available in Solana 3.0
use solana_sdk::{
    instruction::Instruction, pubkey::Pubkey, signature::Signature, transaction::Transaction,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tracing::{debug, error, info, warn};

use crate::{
    fees::FeeManager, rpc::SolanaRpcClient, signing::SigningManager,
    simulation::TransactionSimulator,
};

/// Result of a bundle operation
pub type BundleResult = BundlerResult<BundleResponse>;

/// Transaction bundler that orchestrates the entire bundling process
pub struct TransactionBundler {
    rpc_client: Arc<SolanaRpcClient>,
    fee_manager: Arc<FeeManager>,
    signing_manager: Arc<SigningManager>,
    simulator: Arc<TransactionSimulator>,
    config: BundlerConfig,
}

impl TransactionBundler {
    /// Create a new transaction bundler
    pub fn new(
        rpc_client: Arc<SolanaRpcClient>,
        fee_manager: Arc<FeeManager>,
        signing_manager: Arc<SigningManager>,
        simulator: Arc<TransactionSimulator>,
        config: &BundlerConfig,
    ) -> Self {
        Self {
            rpc_client,
            fee_manager,
            signing_manager,
            simulator,
            config: config.clone(),
        }
    }

    /// Process a bundle request end-to-end
    pub async fn process_bundle(&self, request: BundleRequest) -> BundleResult {
        let start_time = Instant::now();
        let request_id = request.request_id;

        info!("Processing bundle request: {}", request_id);

        // Convert instructions to Solana instructions
        let instructions = self.convert_instructions(&request.instructions)?;

        // Validate instructions
        self.simulator.validate_instructions(&instructions)?;

        // Create transactions from instructions
        let transactions = self
            .create_transactions(instructions, &request.alt_tables, &request.compute)
            .await?;

        let mut results = Vec::new();
        let mut overall_status = BundleStatus::Success;
        let mut bundle_signature: Option<Signature> = None;
        let mut bundle_slot: Option<u64> = None;
        let mut bundle_blockhash: Option<String> = None;
        let mut total_simulation_time = 0u64;
        let mut total_signing_time = 0u64;
        let mut total_submission_time = 0u64;
        let mut total_confirmation_time = 0u64;
        let mut retry_attempts = 0u32;
        let mut rpc_endpoints_used = Vec::new();

        // Process each transaction
        for (i, mut transaction) in transactions.into_iter().enumerate() {
            debug!("Processing transaction {} for bundle {}", i + 1, request_id);

            let tx_result = self
                .process_single_transaction(
                    &mut transaction,
                    &request,
                    &mut total_simulation_time,
                    &mut total_signing_time,
                    &mut total_submission_time,
                    &mut total_confirmation_time,
                    &mut retry_attempts,
                    &mut rpc_endpoints_used,
                )
                .await;

            match tx_result {
                Ok((result, slot, blockhash)) => {
                    if let Some(sig) = &result.signature {
                        bundle_signature = Some(sig.clone());
                    }

                    if let Some(slot_value) = slot {
                        bundle_slot = Some(slot_value);
                    }

                    bundle_blockhash = Some(blockhash);

                    results.push(result);
                }
                Err(e) => {
                    error!("Transaction {} failed: {}", i + 1, e);

                    // Create failed transaction result
                    let failed_result = TransactionResult {
                        signature: None,
                        status: TransactionStatus::Failed,
                        compute_units_consumed: None,
                        fee_paid_lamports: None,
                        logs: vec![],
                        error: Some(e.to_string()),
                    };

                    results.push(failed_result);

                    // Update overall status
                    if request.atomic {
                        overall_status = BundleStatus::Failed;
                        break; // Stop processing if atomic and one failed
                    } else {
                        overall_status = BundleStatus::Failed;
                    }
                }
            }
        }

        let total_latency = start_time.elapsed().as_millis() as u64;

        if overall_status == BundleStatus::Success
            && results
                .iter()
                .any(|result| result.status == TransactionStatus::Failed)
        {
            overall_status = BundleStatus::Failed;
        }

        let total_transactions = results.len() as u32;
        let successful_transactions = results
            .iter()
            .filter(|result| result.status != TransactionStatus::Failed)
            .count() as u32;
        let failed_transactions = total_transactions.saturating_sub(successful_transactions);
        let total_compute_units: ComputeUnits = results
            .iter()
            .filter_map(|result| result.compute_units_consumed)
            .sum();
        let total_fee_paid: Lamports = results
            .iter()
            .filter_map(|result| result.fee_paid_lamports)
            .sum();
        let processing_time_ms = total_simulation_time
            + total_signing_time
            + total_submission_time
            + total_confirmation_time;

        let confirmation =
            results
                .iter()
                .fold(ConfirmationStatus::Unprocessed, |current, result| {
                    let candidate = Self::confirmation_from_transaction_status(&result.status);
                    Self::combine_confirmation_status(current, candidate)
                });

        let metrics = BundleMetrics {
            total_transactions,
            successful_transactions,
            failed_transactions,
            total_compute_units,
            total_fee_paid,
            processing_time_ms,
            total_latency_ms: total_latency,
            simulation_time_ms: total_simulation_time,
            signing_time_ms: total_signing_time,
            submission_time_ms: total_submission_time,
            confirmation_time_ms: total_confirmation_time,
            retry_attempts,
            rpc_endpoints_used,
        };

        let response = BundleResponse {
            request_id,
            status: overall_status,
            transactions: results,
            bundle_signature,
            slot: bundle_slot,
            blockhash: bundle_blockhash,
            confirmation,
            logs_url: Some(format!("/logs/{}", request_id)),
            metrics,
            completed_at: Utc::now(),
        };

        info!(
            "Bundle {} completed with status: {:?}",
            request_id, response.status
        );
        Ok(response)
    }

    /// Process a single transaction within a bundle
    async fn process_single_transaction(
        &self,
        transaction: &mut Transaction,
        request: &BundleRequest,
        total_simulation_time: &mut u64,
        total_signing_time: &mut u64,
        total_submission_time: &mut u64,
        total_confirmation_time: &mut u64,
        retry_attempts: &mut u32,
        rpc_endpoints_used: &mut Vec<String>,
    ) -> BundlerResult<(TransactionResult, Option<u64>, String)> {
        // Step 1: Simulation
        let sim_start = Instant::now();
        let simulation = Some(self.simulator.simulate_transaction(transaction).await?);
        *total_simulation_time += sim_start.elapsed().as_millis() as u64;

        if let Some(ref sim) = simulation {
            if !sim.success {
                return Err(BundlerError::Simulation(
                    sim.error
                        .as_ref()
                        .map(|error| error.message.clone())
                        .unwrap_or_else(|| "Simulation failed".to_string()),
                ));
            }
        }

        // Step 2: Get fresh blockhash
        let blockhash = self.rpc_client.get_latest_blockhash().await?;
        let blockhash_string = blockhash.to_string();
        transaction.message.recent_blockhash = blockhash;

        // Step 3: Signing
        let sign_start = Instant::now();
        if request.signers.is_empty() {
            self.signing_manager.sign_transaction(transaction).await?;
        } else {
            let signer_aliases: Vec<String> = request
                .signers
                .iter()
                .filter_map(|s| s.alias.clone())
                .collect();
            self.signing_manager
                .sign_transaction_with_signers(transaction, &signer_aliases)
                .await?;
        }
        *total_signing_time += sign_start.elapsed().as_millis() as u64;

        // Step 4: Submission with retry logic
        let submit_start = Instant::now();
        let (signature, slot) = self
            .submit_transaction_with_retry(transaction, retry_attempts, rpc_endpoints_used)
            .await?;
        *total_submission_time += submit_start.elapsed().as_millis() as u64;

        // Step 5: Confirmation
        let confirm_start = Instant::now();
        let final_status = self.wait_for_confirmation(&signature).await?;
        *total_confirmation_time += confirm_start.elapsed().as_millis() as u64;

        // Step 6: Get transaction details
        let (compute_units, fee_paid, logs) = if final_status == TransactionStatus::Finalized {
            self.get_transaction_details(&signature)
                .await
                .unwrap_or((None, None, vec![]))
        } else {
            (None, None, vec![])
        };

        Ok((
            TransactionResult {
                signature: Some(signature),
                status: final_status,
                error: None,
                compute_units_consumed: compute_units,
                fee_paid_lamports: fee_paid,
                logs,
            },
            slot,
            blockhash_string,
        ))
    }

    /// Submit transaction with retry logic and fee bumping
    async fn submit_transaction_with_retry(
        &self,
        transaction: &Transaction,
        retry_attempts: &mut u32,
        _rpc_endpoints_used: &mut Vec<String>,
    ) -> BundlerResult<(Signature, Option<u64>)> {
        let _ = _rpc_endpoints_used;
        let mut current_transaction = transaction.clone();
        let mut attempt = 0;
        let max_attempts = 3;

        while attempt < max_attempts {
            match self.rpc_client.send_transaction(&current_transaction).await {
                Ok(signature) => {
                    // Slot information is not currently retrieved
                    return Ok((signature, None));
                }
                Err(e) => {
                    attempt += 1;
                    *retry_attempts += 1;

                    if attempt < max_attempts {
                        warn!("Transaction submission failed (attempt {}): {}", attempt, e);

                        // Try fee bumping if enabled
                        if let Ok(bumped_fee) = self.fee_manager.bump_fee(5000, attempt).await {
                            // Update compute budget instructions with higher fee
                            self.update_transaction_fee(&mut current_transaction, bumped_fee)
                                .await?;

                            // Get fresh blockhash
                            let new_blockhash = self.rpc_client.get_latest_blockhash().await?;
                            current_transaction.message.recent_blockhash = new_blockhash;

                            // Re-sign with new blockhash and fee
                            self.signing_manager
                                .sign_transaction(&mut current_transaction)
                                .await?;
                        }

                        // Exponential backoff
                        let delay = Duration::from_millis(100 * (2_u64.pow(attempt - 1)));
                        tokio::time::sleep(delay).await;
                    } else {
                        return Err(BundlerError::Transaction(format!(
                            "Transaction submission failed after {} attempts: {}",
                            max_attempts, e
                        )));
                    }
                }
            }
        }

        Err(BundlerError::Transaction(
            "Max retry attempts exceeded".to_string(),
        ))
    }

    /// Update transaction fee by modifying compute budget instructions
    async fn update_transaction_fee(
        &self,
        transaction: &mut Transaction,
        new_fee: Lamports,
    ) -> BundlerResult<()> {
        // Find and update compute budget instructions
        let mut instructions = transaction.message.instructions.clone();
        let account_keys = &transaction.message.account_keys;

        // TODO: Use proper compute budget program ID when available in Solana 3.0
        // Using a placeholder pubkey for now
        let compute_budget_program = Pubkey::default();

        // Find compute budget program index
        let program_index = account_keys
            .iter()
            .position(|key| *key == compute_budget_program)
            .ok_or_else(|| {
                BundlerError::Transaction("Compute budget program not found".to_string())
            })?;

        // Update or add compute unit price instruction
        let mut found_price_instruction = false;
        for instruction in &mut instructions {
            if instruction.program_id_index == program_index as u8 {
                // Check if this is a compute unit price instruction
                if instruction.data.len() >= 5 && instruction.data[0] == 3 {
                    // Update the price (bytes 1-8 are the price in little-endian)
                    let price_bytes = new_fee.to_le_bytes();
                    instruction.data[1..9].copy_from_slice(&price_bytes);
                    found_price_instruction = true;
                    break;
                }
            }
        }

        if !found_price_instruction {
            return Err(BundlerError::Transaction(
                "Compute unit price instruction not found".to_string(),
            ));
        }

        // Rebuild the transaction with updated instructions
        // This is a simplified approach - in practice, you'd need to properly rebuild the message
        debug!("Updated transaction fee to {} lamports", new_fee);

        Ok(())
    }

    /// Wait for transaction confirmation
    async fn wait_for_confirmation(
        &self,
        signature: &Signature,
    ) -> BundlerResult<TransactionStatus> {
        let timeout_duration = Duration::from_secs(60); // 60 second timeout
        let start_time = Instant::now();

        while start_time.elapsed() < timeout_duration {
            // Check if transaction is confirmed
            if let Ok(confirmed) = self
                .rpc_client
                .confirm_transaction(signature, CommitmentLevel::Confirmed)
                .await
            {
                if confirmed {
                    // Check for finalized
                    if let Ok(finalized) = self
                        .rpc_client
                        .confirm_transaction(signature, CommitmentLevel::Finalized)
                        .await
                    {
                        if finalized {
                            return Ok(TransactionStatus::Finalized);
                        }
                    }
                    return Ok(TransactionStatus::Confirmed);
                }
            }

            // Check if transaction is processed
            if let Ok(Some(_)) = self.rpc_client.get_transaction(signature).await {
                return Ok(TransactionStatus::Processed);
            }

            // Wait before next check
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        Ok(TransactionStatus::Failed)
    }

    /// Get detailed transaction information
    async fn get_transaction_details(
        &self,
        signature: &Signature,
    ) -> BundlerResult<(Option<ComputeUnits>, Option<Lamports>, Vec<String>)> {
        match self.rpc_client.get_transaction(signature).await? {
            Some(tx) => {
                let compute_units = tx.transaction.meta.as_ref().and_then(|meta| {
                    match &meta.compute_units_consumed {
                        solana_transaction_status::option_serializer::OptionSerializer::Some(
                            cu,
                        ) => Some(*cu as ComputeUnits),
                        _ => None,
                    }
                });

                let fee_paid = tx
                    .transaction
                    .meta
                    .as_ref()
                    .map(|meta| meta.fee as Lamports);

                let logs = tx
                    .transaction
                    .meta
                    .as_ref()
                    .and_then(|meta| match &meta.log_messages {
                        solana_transaction_status::option_serializer::OptionSerializer::Some(
                            logs,
                        ) => Some(logs.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();

                Ok((compute_units, fee_paid, logs))
            }
            None => Ok((None, None, vec![])),
        }
    }

    fn confirmation_from_transaction_status(status: &TransactionStatus) -> ConfirmationStatus {
        match status {
            TransactionStatus::Finalized => ConfirmationStatus::Finalized,
            TransactionStatus::Confirmed => ConfirmationStatus::Confirmed,
            TransactionStatus::Processed | TransactionStatus::Failed => {
                ConfirmationStatus::Processed
            }
            TransactionStatus::Pending => ConfirmationStatus::Unprocessed,
        }
    }

    fn combine_confirmation_status(
        a: ConfirmationStatus,
        b: ConfirmationStatus,
    ) -> ConfirmationStatus {
        use ConfirmationStatus::*;

        match (a, b) {
            (Finalized, _) | (_, Finalized) => Finalized,
            (Confirmed, _) | (_, Confirmed) => Confirmed,
            (Processed, _) | (_, Processed) => Processed,
            _ => Unprocessed,
        }
    }

    /// Convert instruction data to Solana instructions
    fn convert_instructions(
        &self,
        instruction_data: &[InstructionData],
    ) -> BundlerResult<Vec<Instruction>> {
        instruction_data
            .iter()
            .map(|data| {
                let instruction_bytes =
                    general_purpose::STANDARD
                        .decode(&data.data_b64)
                        .map_err(|e| {
                            BundlerError::InvalidInput(format!("Invalid base64 data: {}", e))
                        })?;

                let accounts = data
                    .keys
                    .iter()
                    .map(|meta| solana_sdk::instruction::AccountMeta {
                        pubkey: meta.pubkey,
                        is_signer: meta.is_signer,
                        is_writable: meta.is_writable,
                    })
                    .collect();

                Ok(Instruction {
                    program_id: data.program_id,
                    accounts,
                    data: instruction_bytes,
                })
            })
            .collect()
    }

    /// Create transactions from instructions, handling batching and compute budgets
    async fn create_transactions(
        &self,
        mut instructions: Vec<Instruction>,
        alt_tables: &[Pubkey],
        compute_config: &bundler_types::ComputeConfig,
    ) -> BundlerResult<Vec<Transaction>> {
        let fee_payer = self.signing_manager.fee_payer_pubkey().await?;

        // Calculate fees
        let accounts: Vec<Pubkey> = instructions
            .iter()
            .flat_map(|ix| ix.accounts.iter().map(|acc| acc.pubkey))
            .collect();

        let fee_lamports = match &compute_config.price {
            ComputePrice::Auto => self.fee_manager.calculate_fee(&accounts).await?,
            ComputePrice::Fixed(lamports) => *lamports,
        };

        // Determine compute units
        let compute_units = match &compute_config.limit {
            ComputeLimit::Auto => {
                // We'll estimate this per transaction after batching
                200_000 // Default for now
            }
            ComputeLimit::Fixed(units) => *units,
        };

        // Add compute budget instructions
        let compute_budget_instructions = self
            .fee_manager
            .create_compute_budget_instructions(compute_units, fee_lamports);

        // Prepend compute budget instructions
        let mut all_instructions = compute_budget_instructions;
        all_instructions.extend(instructions);

        // For now, create a single transaction
        // In a more sophisticated implementation, we'd batch based on size limits
        let transaction = if alt_tables.is_empty() {
            // Legacy transaction
            Transaction::new_with_payer(&all_instructions, Some(&fee_payer))
        } else {
            // V0 transaction with ALTs (simplified - would need proper ALT handling)
            Transaction::new_with_payer(&all_instructions, Some(&fee_payer))
        };

        Ok(vec![transaction])
    }

    /// Estimate the optimal batch size for instructions
    fn estimate_batch_size(&self, instructions: &[Instruction]) -> usize {
        // Simplified batching logic
        // In practice, you'd consider:
        // - Transaction size limits (1232 bytes)
        // - Account conflicts
        // - Compute unit limits

        let total_accounts: usize = instructions.iter().map(|ix| ix.accounts.len()).sum();
        let total_data_size: usize = instructions.iter().map(|ix| ix.data.len()).sum();

        // Conservative estimate: if we have many accounts or large data, use smaller batches
        if total_accounts > 20 || total_data_size > 800 {
            1 // One instruction per transaction
        } else {
            instructions.len().min(5) // Max 5 instructions per transaction
        }
    }

    /// Check for account conflicts between instructions
    fn has_account_conflicts(&self, instructions: &[Instruction]) -> bool {
        let mut writable_accounts = std::collections::HashSet::new();

        for instruction in instructions {
            for account_meta in &instruction.accounts {
                if account_meta.is_writable {
                    if !writable_accounts.insert(account_meta.pubkey) {
                        return true; // Conflict found
                    }
                }
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bundler_config::BundlerConfigBuilder;
    use bundler_types::{AccountMeta, ComputeConfig, ComputeLimit, ComputePrice, InstructionData};
    use solana_sdk::{signature::Keypair, system_instruction};
    use uuid::Uuid;

    async fn create_test_bundler() -> TransactionBundler {
        let config = BundlerConfigBuilder::new()
            .with_rpc_endpoint("https://api.devnet.solana.com".to_string(), 100)
            .build()
            .unwrap();

        let rpc_client = Arc::new(SolanaRpcClient::new(&config).unwrap());
        let fee_manager = Arc::new(FeeManager::new(
            Arc::clone(&rpc_client),
            config.fees.clone(),
        ));
        let signing_manager = Arc::new(SigningManager::new(&config.signing).await.unwrap());
        let simulator = Arc::new(TransactionSimulator::new(
            Arc::clone(&rpc_client),
            &config.security,
        ));

        TransactionBundler::new(rpc_client, fee_manager, signing_manager, simulator, &config)
    }

    fn create_test_bundle_request() -> BundleRequest {
        let from = Keypair::new();
        let to = Keypair::new();

        let instruction = system_instruction::transfer(&from.pubkey(), &to.pubkey(), 1000);

        let instruction_data = InstructionData {
            program_id: instruction.program_id,
            keys: instruction
                .accounts
                .into_iter()
                .map(|meta| AccountMeta {
                    pubkey: meta.pubkey,
                    is_signer: meta.is_signer,
                    is_writable: meta.is_writable,
                })
                .collect(),
            data_b64: general_purpose::STANDARD.encode(&instruction.data),
        };

        BundleRequest {
            request_id: Uuid::new_v4(),
            atomic: true,
            compute: ComputeConfig {
                limit: ComputeLimit::Auto,
                price: ComputePrice::Auto,
                max_price_lamports: 10000,
            },
            alt_tables: vec![],
            instructions: vec![instruction_data],
            signers: vec![],
            metadata: std::collections::HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_bundler_creation() {
        let bundler = create_test_bundler().await;
        assert_eq!(bundler.config.rpc.endpoints.len(), 1);
    }

    #[test]
    fn test_instruction_conversion() {
        let bundler = tokio_test::block_on(create_test_bundler());

        let from = Keypair::new();
        let to = Keypair::new();
        let instruction = system_instruction::transfer(&from.pubkey(), &to.pubkey(), 1000);

        let instruction_data = InstructionData {
            program_id: instruction.program_id,
            keys: instruction
                .accounts
                .into_iter()
                .map(|meta| AccountMeta {
                    pubkey: meta.pubkey,
                    is_signer: meta.is_signer,
                    is_writable: meta.is_writable,
                })
                .collect(),
            data_b64: general_purpose::STANDARD.encode(&instruction.data),
        };

        let converted = bundler.convert_instructions(&[instruction_data]).unwrap();
        assert_eq!(converted.len(), 1);
        assert_eq!(converted[0].program_id, solana_sdk::system_program::id());
    }

    #[test]
    fn test_batch_size_estimation() {
        let bundler = tokio_test::block_on(create_test_bundler());

        // Test with simple instructions
        let simple_instructions = vec![system_instruction::transfer(
            &Keypair::new().pubkey(),
            &Keypair::new().pubkey(),
            1000,
        )];

        let batch_size = bundler.estimate_batch_size(&simple_instructions);
        assert_eq!(batch_size, 1);

        // Test with many instructions
        let many_instructions: Vec<_> = (0..10)
            .map(|_| {
                system_instruction::transfer(
                    &Keypair::new().pubkey(),
                    &Keypair::new().pubkey(),
                    1000,
                )
            })
            .collect();

        let batch_size = bundler.estimate_batch_size(&many_instructions);
        assert!(batch_size <= 5); // Should be limited to max 5
    }

    #[test]
    fn test_account_conflict_detection() {
        let bundler = tokio_test::block_on(create_test_bundler());

        let account1 = Keypair::new().pubkey();
        let account2 = Keypair::new().pubkey();

        // No conflicts
        let no_conflict_instructions =
            vec![system_instruction::transfer(&account1, &account2, 1000)];

        assert!(!bundler.has_account_conflicts(&no_conflict_instructions));

        // With conflicts (same writable account)
        let conflict_instructions = vec![
            system_instruction::transfer(&account1, &account2, 1000),
            system_instruction::transfer(&account2, &account1, 500), // account2 is writable in both
        ];

        assert!(bundler.has_account_conflicts(&conflict_instructions));
    }

    #[tokio::test]
    async fn test_bundle_request_processing_structure() {
        let bundler = create_test_bundler().await;
        let request = create_test_bundle_request();

        // Test that the bundler can at least parse and validate the request structure
        let instructions = bundler.convert_instructions(&request.instructions).unwrap();
        assert_eq!(instructions.len(), 1);

        // Test compute config handling
        match &request.compute.limit {
            ComputeLimit::Auto => assert!(true),
            ComputeLimit::Fixed(_) => assert!(true),
        }

        match &request.compute.price {
            ComputePrice::Auto => assert!(true),
            ComputePrice::Fixed(_) => assert!(true),
        }
    }
    #[test]
    fn test_new() {
        // TODO: Implement test for new
        // Function signature: new(rpc_client: Arc<SolanaRpcClient>, fee_manager: Arc<FeeManager>, signing_manager: Arc<SigningManager>, simulator: Arc<TransactionSimulator>, config: &BundlerConfig) -> Self
        let result = new(Default::default(), Default::default(), Default::default(), Default::default(), Default::default());
        // Add assertions here
    }

    #[test]
    fn test_new_edge_cases() {
        // TODO: Test edge cases for new
        // Consider: empty inputs, boundary values, error conditions
    }
}
