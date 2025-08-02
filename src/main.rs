use anyhow::Result;
use clap::Parser;
use std::sync::Arc;
use tokio::signal;
use tracing::{info, warn};

mod openstack;
mod metrics;
mod ml;
mod scheduler;
mod config;
mod error;

use crate::config::Config;
use crate::metrics::MetricsCollector;
use crate::ml::MLEngine;
use crate::scheduler::ResourceScheduler;

#[derive(Parser)]
#[command(name = "openstack-metrics-service")]
#[command(about = "High-performance OpenStack metrics collection and ML-based resource scheduling")]
struct Cli {
    #[arg(short, long, default_value = "config.toml")]
    config: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    let cli = Cli::parse();
    let config = Config::from_file(&cli.config)?;
    
    info!("Starting OpenStack Metrics Service");
    
    // Initialize core components
    let openstack_client = Arc::new(
        openstack::Client::new(&config.openstack).await?
    );
    
    let metrics_collector = Arc::new(
        MetricsCollector::new(&config.metrics, openstack_client.clone()).await?
    );
    
    let ml_engine = Arc::new(
        MLEngine::new(&config.ml).await?
    );
    
    let scheduler = Arc::new(
        ResourceScheduler::new(
            &config.scheduler,
            openstack_client.clone(),
            ml_engine.clone()
        ).await?
    );
    
    // Start services
    let metrics_handle = tokio::spawn({
        let collector = metrics_collector.clone();
        async move {
            if let Err(e) = collector.start_collection().await {
                warn!("Metrics collection error: {}", e);
            }
        }
    });
    
    let ml_handle = tokio::spawn({
        let engine = ml_engine.clone();
        async move {
            if let Err(e) = engine.start_inference_loop().await {
                warn!("ML engine error: {}", e);
            }
        }
    });
    
    let scheduler_handle = tokio::spawn({
        let sched = scheduler.clone();
        async move {
            if let Err(e) = sched.start_scheduling_loop().await {
                warn!("Scheduler error: {}", e);
            }
        }
    });
    
    info!("All services started successfully");
    
    // Wait for shutdown signal
    signal::ctrl_c().await?;
    info!("Shutdown signal received, stopping services...");
    
    // Graceful shutdown
    metrics_handle.abort();
    ml_handle.abort();
    scheduler_handle.abort();
    
    Ok(())
}
