use anyhow::Result;
use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use serde_json;
use std::time::Duration;
use tracing::{debug, error};

use crate::config::KafkaConfig;
use crate::openstack::services::{ServerMetrics, NetworkMetrics, StorageMetrics};

#[derive(Clone)]
pub struct KafkaProducer {
    producer: FutureProducer,
    config: KafkaConfig,
}

impl KafkaProducer {
    pub async fn new(config: &KafkaConfig) -> Result<Self> {
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", &config.brokers)
            .set("message.timeout.ms", "5000")
            .set("queue.buffering.max.messages", "100000")
            .set("queue.buffering.max.ms", "10")
            .set("batch.num.messages", "1000")
            .create()?;
        
        Ok(Self {
            producer,
            config: config.clone(),
        })
    }
    
    pub async fn send_server_metrics(&self, metrics: &ServerMetrics) -> Result<()> {
        let payload = serde_json::to_string(metrics)?;
        
        let record = FutureRecord::to(&self.config.compute_topic)
            .key(&metrics.server_id)
            .payload(&payload);
        
        match self.producer.send(record, Duration::from_secs(1)).await {
            Ok(_) => {
                debug!("Sent server metrics for {}", metrics.server_id);
                Ok(())
            },
            Err((e, _)) => {
                error!("Failed to send server metrics: {}", e);
                Err(e.into())
            }
        }
    }
    
    pub async fn send_network_metrics(&self, metrics: &NetworkMetrics) -> Result<()> {
        let payload = serde_json::to_string(metrics)?;
        
        let record = FutureRecord::to(&self.config.network_topic)
            .key(&metrics.network_id)
            .payload(&payload);
        
        match self.producer.send(record, Duration::from_secs(1)).await {
            Ok(_) => {
                debug!("Sent network metrics for {}", metrics.network_id);
                Ok(())
            },
            Err((e, _)) => {
                error!("Failed to send network metrics: {}", e);
                Err(e.into())
            }
        }
    }
    
    pub async fn send_storage_metrics(&self, metrics: &StorageMetrics) -> Result<()> {
        let payload = serde_json::to_string(metrics)?;
        
        let record = FutureRecord::to(&self.config.storage_topic)
            .key(&metrics.volume_id)
            .payload(&payload);
        
        match self.producer.send(record, Duration::from_secs(1)).await {
            Ok(_) => {
                debug!("Sent storage metrics for {}", metrics.volume_id);
                Ok(())
            },
            Err((e, _)) => {
                error!("Failed to send storage metrics: {}", e);
                Err(e.into())
            }
        }
    }
}
