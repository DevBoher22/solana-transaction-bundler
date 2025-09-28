use bundler_config::BundlerConfig;
use bundler_service::start_service;
use clap::Parser;
use std::path::PathBuf;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Solana Transaction Bundler HTTP Service
#[derive(Parser)]
#[command(name = "bundler-service")]
#[command(about = "HTTP service for the Solana transaction bundler")]
#[command(version = "0.1.0")]
struct Cli {
    /// Configuration file path
    #[arg(short, long, default_value = "bundler.config.toml")]
    config: PathBuf,
    
    /// Log level (trace, debug, info, warn, error)
    #[arg(short, long, default_value = "info")]
    log_level: String,
    
    /// Port to listen on (overrides config)
    #[arg(short, long)]
    port: Option<u16>,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    
    // Initialize logging
    let level_filter = match cli.log_level.to_lowercase().as_str() {
        "trace" => tracing::Level::TRACE,
        "debug" => tracing::Level::DEBUG,
        "info" => tracing::Level::INFO,
        "warn" => tracing::Level::WARN,
        "error" => tracing::Level::ERROR,
        _ => {
            eprintln!("Invalid log level: {}", cli.log_level);
            std::process::exit(1);
        }
    };
    
    tracing_subscriber::registry()
        .with(tracing_subscriber::filter::LevelFilter::from_level(level_filter))
        .with(tracing_subscriber::fmt::layer().json())
        .init();
    
    // Load configuration
    let mut config = if cli.config.exists() {
        match BundlerConfig::load_from_path(&cli.config) {
            Ok(config) => config,
            Err(e) => {
                error!("Failed to load configuration: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        info!("Configuration file not found, using defaults");
        BundlerConfig::default()
    };
    
    // Override port if specified
    if let Some(port) = cli.port {
        config.service.port = port;
    }
    
    // Validate configuration
    if let Err(e) = config.validate() {
        error!("Configuration validation failed: {}", e);
        std::process::exit(1);
    }
    
    info!("Starting Solana Transaction Bundler Service");
    info!("Configuration loaded from: {}", cli.config.display());
    info!("Service will listen on port: {}", config.service.port);
    
    // Start the service
    if let Err(e) = start_service(config).await {
        error!("Service failed: {}", e);
        std::process::exit(1);
    }
}
