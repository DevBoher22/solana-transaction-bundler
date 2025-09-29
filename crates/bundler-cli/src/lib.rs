use anyhow::{Context, Result};
use bundler_config::BundlerConfig;
use bundler_core::BundlerService;
use bundler_types::{BundleRequest, BundleStatus, TransactionStatus};
use chrono::Utc;
use clap::{Parser, Subcommand};
use solana_commitment_config::CommitmentLevel;
use solana_sdk::instruction::Instruction;
use solana_transaction_status::option_serializer::OptionSerializer;
use std::path::{Path, PathBuf};
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Solana Transaction Bundler CLI
#[derive(Parser)]
#[command(name = "bundler")]
#[command(
    about = "A production-ready Solana transaction bundler with low latency and high success rate"
)]
#[command(version = "0.1.0")]
pub struct Cli {
    /// Configuration file path
    #[arg(short, long, default_value = "bundler.config.toml")]
    pub config: PathBuf,

    /// Log level (trace, debug, info, warn, error)
    #[arg(short, long, default_value = "info")]
    pub log_level: String,

    /// Log format (json, pretty)
    #[arg(long, default_value = "pretty")]
    pub log_format: String,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Simulate transactions without submitting them
    Simulate {
        /// Path to JSON file containing bundle request
        #[arg(value_name = "FILE")]
        file: PathBuf,

        /// Show detailed logs
        #[arg(short, long)]
        verbose: bool,
    },

    /// Submit a bundle of transactions
    Submit {
        /// Path to JSON file containing bundle request
        #[arg(value_name = "FILE")]
        file: PathBuf,

        /// Force atomic execution (all transactions must succeed)
        #[arg(short, long)]
        atomic: bool,

        /// Override compute unit limit
        #[arg(long)]
        cu_limit: Option<u32>,

        /// Override compute unit price strategy (auto or specific lamports)
        #[arg(long)]
        cu_price: Option<String>,

        /// Wait for finalization before returning
        #[arg(short, long)]
        wait: bool,

        /// Timeout in seconds for waiting
        #[arg(long, default_value = "60")]
        timeout: u64,
    },

    /// Check the status of a transaction or bundle
    Status {
        /// Transaction signature or request ID
        #[arg(value_name = "ID")]
        id: String,

        /// Show detailed information
        #[arg(short, long)]
        verbose: bool,
    },

    /// Show health status of the bundler service
    Health {
        /// Show detailed component status
        #[arg(short, long)]
        verbose: bool,
    },

    /// Show configuration and validate settings
    Config {
        /// Show the current configuration
        #[arg(short, long)]
        show: bool,

        /// Validate configuration without starting service
        #[arg(short, long)]
        validate: bool,
    },
}

pub struct CliRunner {
    config: BundlerConfig,
    service: BundlerService,
}

impl CliRunner {
    /// Create a new CLI runner
    pub async fn new(config_path: &PathBuf) -> Result<Self> {
        let config = if config_path.exists() {
            BundlerConfig::load_from_path(config_path).context("Failed to load configuration")?
        } else {
            warn!("Configuration file not found, using defaults");
            BundlerConfig::default()
        };

        let service = BundlerService::new(config.clone())
            .await
            .context("Failed to initialize bundler service")?;

        Ok(Self { config, service })
    }

    /// Run the CLI command
    pub async fn run(&self, command: Commands) -> Result<()> {
        match command {
            Commands::Simulate { file, verbose } => self.simulate_command(file, verbose).await,
            Commands::Submit {
                file,
                atomic,
                cu_limit,
                cu_price,
                wait,
                timeout,
            } => {
                self.submit_command(file, atomic, cu_limit, cu_price, wait, timeout)
                    .await
            }
            Commands::Status { id, verbose } => self.status_command(id, verbose).await,
            Commands::Health { verbose } => self.health_command(verbose).await,
            Commands::Config { show, validate } => self.config_command(show, validate).await,
        }
    }

    /// Handle simulate command
    async fn simulate_command(&self, file: PathBuf, verbose: bool) -> Result<()> {
        info!("Simulating bundle from file: {}", file.display());

        let bundle_request = load_bundle_request(&file)?;

        // Create transactions from the request
        let instructions: Vec<Instruction> = bundle_request
            .instructions
            .iter()
            .map(|ix| ix.clone().into())
            .collect();

        // Simulate each instruction
        for (i, instruction) in instructions.iter().enumerate() {
            println!("Simulating instruction {} of {}", i + 1, instructions.len());

            // Create a simple transaction for simulation
            let fee_payer = self.service.get_fee_payer_pubkey().await?;
            let mut transaction = solana_sdk::transaction::Transaction::new_with_payer(
                std::slice::from_ref(instruction),
                Some(&fee_payer),
            );

            // Set a dummy blockhash for simulation
            transaction.message.recent_blockhash = solana_sdk::hash::Hash::new_unique();

            match self.service.simulate_transaction(&transaction).await {
                Ok(result) => {
                    println!("✅ Simulation successful");
                    if let Some(cu) = result.compute_units_consumed {
                        println!("   Compute units: {}", cu);
                    }
                    if let Some(fee) = result.estimated_fee {
                        println!("   Estimated fee: {} lamports", fee);
                    }

                    if verbose && !result.logs.is_empty() {
                        println!("   Logs:");
                        for log in &result.logs {
                            println!("     {}", log);
                        }
                    }
                }
                Err(e) => {
                    println!("❌ Simulation failed: {}", e);
                    if verbose {
                        println!("   Error details: {:?}", e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Handle submit command
    async fn submit_command(
        &self,
        file: PathBuf,
        atomic: bool,
        cu_limit: Option<u32>,
        cu_price: Option<String>,
        wait: bool,
        timeout: u64,
    ) -> Result<()> {
        info!("Submitting bundle from file: {}", file.display());

        let mut bundle_request = load_bundle_request(&file)?;

        // Override settings from command line
        if atomic {
            bundle_request.atomic = true;
        }

        if let Some(limit) = cu_limit {
            bundle_request.compute.limit = bundler_types::ComputeLimit::Fixed(limit);
        }

        if let Some(price_str) = cu_price {
            bundle_request.compute.price = if price_str == "auto" {
                bundler_types::ComputePrice::Auto
            } else {
                let price: u64 = price_str.parse().context("Invalid compute unit price")?;
                bundler_types::ComputePrice::Fixed(price)
            };
        }

        // Submit the bundle
        match self.service.process_bundle(bundle_request).await {
            Ok(response) => {
                println!("Bundle submitted successfully!");
                println!("Request ID: {}", response.request_id);
                println!("Status: {:?}", response.status);
                println!("Transactions: {}", response.transactions.len());
                if let Some(slot) = response.slot {
                    println!("Slot: {}", slot);
                }

                // Show transaction details
                for (i, tx_result) in response.transactions.iter().enumerate() {
                    println!("  Transaction {}:", i + 1);
                    if let Some(sig) = &tx_result.signature {
                        println!("    Signature: {}", sig);
                    }
                    println!("    Status: {:?}", tx_result.status);

                    if let Some(fee) = tx_result.fee_paid_lamports {
                        println!("    Fee paid: {} lamports", fee);
                    }

                    if let Some(cu) = tx_result.compute_units_consumed {
                        println!("    Compute units: {}", cu);
                    }

                    if !tx_result.logs.is_empty() {
                        println!("    Logs:");
                        for log in &tx_result.logs {
                            println!("      {}", log);
                        }
                    }

                    if let Some(error) = &tx_result.error {
                        println!("    Error: {}", error);
                    }
                }

                // Show metrics
                println!("\nMetrics:");
                println!("  Total latency: {}ms", response.metrics.total_latency_ms);
                println!(
                    "  Simulation time: {}ms",
                    response.metrics.simulation_time_ms
                );
                println!("  Signing time: {}ms", response.metrics.signing_time_ms);
                println!(
                    "  Submission time: {}ms",
                    response.metrics.submission_time_ms
                );
                println!(
                    "  Confirmation time: {}ms",
                    response.metrics.confirmation_time_ms
                );
                println!("  Retry attempts: {}", response.metrics.retry_attempts);

                // Wait for finalization if requested
                if wait && response.status != BundleStatus::Failed {
                    println!("\nWaiting for finalization...");
                    self.wait_for_finalization(&response.transactions, timeout)
                        .await?;
                }

                // Exit with error code if bundle failed
                if response.status == BundleStatus::Failed {
                    std::process::exit(1);
                }
            }
            Err(e) => {
                error!("Bundle submission failed: {}", e);
                std::process::exit(1);
            }
        }

        Ok(())
    }

    /// Handle status command
    async fn status_command(&self, id: String, verbose: bool) -> Result<()> {
        info!("Checking status for: {}", id);

        // Try to parse as signature first
        if let Ok(signature) = id.parse::<solana_sdk::signature::Signature>() {
            match self.service.get_transaction(&signature).await {
                Ok(Some(tx)) => {
                    println!("Transaction found: {}", signature);

                    if let Some(meta) = &tx.transaction.meta {
                        println!(
                            "Status: {:?}",
                            if meta.err.is_none() {
                                TransactionStatus::Finalized
                            } else {
                                TransactionStatus::Failed
                            }
                        );

                        println!("Slot: {}", tx.slot);

                        println!("Fee: {} lamports", meta.fee);

                        if let OptionSerializer::Some(cu) = meta.compute_units_consumed {
                            println!("Compute units: {}", cu);
                        }

                        if verbose {
                            if let OptionSerializer::Some(logs) = &meta.log_messages {
                                println!("\nLogs:");
                                for log in logs {
                                    println!("  {}", log);
                                }
                            }
                        }

                        if let Some(err) = &meta.err {
                            println!("Error: {:?}", err);
                        }
                    }
                }
                Ok(None) => {
                    println!("Transaction not found: {}", signature);
                }
                Err(e) => {
                    error!("Failed to get transaction: {}", e);
                    std::process::exit(1);
                }
            }
        } else {
            // Try as request ID (UUID)
            println!("Request ID status checking not implemented yet");
            println!("Use transaction signature instead");
        }

        Ok(())
    }

    /// Handle health command
    async fn health_command(&self, verbose: bool) -> Result<()> {
        info!("Checking bundler health");

        match self.service.health_check().await {
            Ok(health) => {
                let all_healthy = health.values().all(|status| status == "healthy");
                if all_healthy {
                    println!("✅ Bundler is healthy");
                } else {
                    println!("❌ Bundler is unhealthy");
                }

                println!("Last check: {}", Utc::now().to_rfc3339());

                if verbose {
                    println!("\nComponent status:");
                    for (name, status) in &health {
                        let status_icon = if status == "healthy" { "✅" } else { "❌" };
                        println!("  {} {} ({})", status_icon, name, status);
                    }
                }

                if !all_healthy {
                    std::process::exit(1);
                }
            }
            Err(e) => {
                error!("Health check failed: {}", e);
                std::process::exit(1);
            }
        }

        Ok(())
    }

    /// Handle config command
    async fn config_command(&self, show: bool, validate: bool) -> Result<()> {
        if show {
            println!("Current configuration:");
            println!("{}", toml::to_string_pretty(&self.config)?);
        }

        if validate {
            match self.config.validate() {
                Ok(_) => println!("✅ Configuration is valid"),
                Err(e) => {
                    println!("❌ Configuration is invalid: {}", e);
                    std::process::exit(1);
                }
            }
        }

        if !show && !validate {
            println!("Use --show to display configuration or --validate to check it");
        }

        Ok(())
    }

    /// Wait for transactions to be finalized
    async fn wait_for_finalization(
        &self,
        transactions: &[bundler_types::TransactionResult],
        timeout_seconds: u64,
    ) -> Result<()> {
        use tokio::time::{sleep, timeout, Duration};

        let timeout_duration = Duration::from_secs(timeout_seconds);
        let start_time = std::time::Instant::now();

        for tx_result in transactions {
            if tx_result.status == TransactionStatus::Failed {
                continue; // Skip failed transactions
            }

            let Some(signature) = &tx_result.signature else {
                warn!("Skipping transaction without signature when waiting for finalization");
                continue;
            };

            let remaining_time = timeout_duration.saturating_sub(start_time.elapsed());
            if remaining_time.is_zero() {
                warn!("Timeout waiting for finalization");
                break;
            }

            println!("Waiting for transaction {} to finalize...", signature);

            let result = timeout(remaining_time, async {
                loop {
                    match self
                        .service
                        .confirm_transaction(signature, CommitmentLevel::Finalized)
                        .await
                    {
                        Ok(true) => {
                            println!("✅ Transaction {} finalized", signature);
                            return Ok::<(), bundler_types::BundlerError>(());
                        }
                        Ok(false) => {
                            sleep(Duration::from_millis(500)).await;
                        }
                        Err(e) => {
                            warn!("Error checking transaction status: {}", e);
                            sleep(Duration::from_secs(1)).await;
                        }
                    }
                }
            })
            .await;

            match result {
                Ok(_) => {} // Transaction finalized
                Err(_) => {
                    warn!("Timeout waiting for transaction {} to finalize", signature);
                }
            }
        }

        Ok(())
    }
}

/// Load bundle request from JSON file
fn load_bundle_request(path: impl AsRef<Path>) -> Result<BundleRequest> {
    let path = path.as_ref();
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;

    let request: BundleRequest = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse JSON from: {}", path.display()))?;

    Ok(request)
}

/// Initialize logging based on configuration
pub fn init_logging(level: &str, format: &str) -> Result<()> {
    let level_filter = match level.to_lowercase().as_str() {
        "trace" => tracing::Level::TRACE,
        "debug" => tracing::Level::DEBUG,
        "info" => tracing::Level::INFO,
        "warn" => tracing::Level::WARN,
        "error" => tracing::Level::ERROR,
        _ => return Err(anyhow::anyhow!("Invalid log level: {}", level)),
    };

    let subscriber = tracing_subscriber::registry().with(
        tracing_subscriber::filter::LevelFilter::from_level(level_filter),
    );

    match format.to_lowercase().as_str() {
        "json" => {
            subscriber
                .with(tracing_subscriber::fmt::layer().json())
                .init();
        }
        "pretty" => {
            subscriber
                .with(tracing_subscriber::fmt::layer().pretty())
                .init();
        }
        _ => return Err(anyhow::anyhow!("Invalid log format: {}", format)),
    }

    Ok(())
}

/// Main CLI entry point
pub async fn run_cli() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    init_logging(&cli.log_level, &cli.log_format)?;

    // Create and run CLI
    let runner = CliRunner::new(&cli.config).await?;
    runner.run(cli.command).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use bundler_types::{ComputeConfig, ComputeLimit, ComputePrice, InstructionData};
    use solana_sdk::{instruction::AccountMeta, pubkey::Pubkey};
    use std::io::Write;
    use tempfile::NamedTempFile;
    use uuid::Uuid;

    #[test]
    fn parses_submit_command() {
        let args = vec!["bundler", "submit", "bundle.json"];
        let cli = Cli::try_parse_from(args).unwrap();

        assert_eq!(cli.config, PathBuf::from("bundler.config.toml"));
        match cli.command {
            Commands::Submit {
                file,
                atomic,
                cu_limit,
                cu_price,
                wait,
                timeout,
            } => {
                assert_eq!(file, PathBuf::from("bundle.json"));
                assert!(!atomic);
                assert!(cu_limit.is_none());
                assert!(cu_price.is_none());
                assert!(!wait);
                assert_eq!(timeout, 60);
            }
            _ => panic!("expected submit command"),
        }
    }

    #[test]
    fn parses_status_verbose_flag() {
        let args = vec!["bundler", "status", "sig", "--verbose"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Commands::Status { id, verbose } => {
                assert_eq!(id, "sig");
                assert!(verbose);
            }
            _ => panic!("expected status command"),
        }
    }

    #[test]
    fn parses_config_flags() {
        let args = vec!["bundler", "config", "--show", "--validate"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Commands::Config { show, validate } => {
                assert!(show);
                assert!(validate);
            }
            _ => panic!("expected config command"),
        }
    }

    #[test]
    fn loads_bundle_request_from_file() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let bundle_request = BundleRequest {
            request_id: Uuid::new_v4(),
            atomic: true,
            compute: ComputeConfig {
                limit: ComputeLimit::Fixed(200_000),
                price: ComputePrice::Fixed(1_000),
                max_price_lamports: 50_000,
            },
            alt_tables: vec![],
            instructions: vec![InstructionData {
                program_id: Pubkey::new_unique(),
                keys: vec![AccountMeta {
                    pubkey: Pubkey::new_unique(),
                    is_signer: true,
                    is_writable: true,
                }],
                data_b64: STANDARD.encode([1u8, 2, 3, 4]),
            }],
            signers: vec![],
            metadata: std::collections::HashMap::new(),
        };

        let json = serde_json::to_string_pretty(&bundle_request).unwrap();
        temp_file.write_all(json.as_bytes()).unwrap();
        temp_file.flush().unwrap();

        let loaded_request = load_bundle_request(temp_file.path()).unwrap();
        assert_eq!(bundle_request.request_id, loaded_request.request_id);
        assert_eq!(bundle_request.atomic, loaded_request.atomic);
        assert_eq!(
            bundle_request.instructions.len(),
            loaded_request.instructions.len()
        );
    }

    #[test]
    fn rejects_invalid_bundle_file() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"invalid json").unwrap();
        temp_file.flush().unwrap();

        let result = load_bundle_request(temp_file.path());
        assert!(result.is_err());
    }

    #[test]
    fn rejects_missing_bundle_file() {
        let result = load_bundle_request("/nonexistent/file.json");
        assert!(result.is_err());
    }
}
