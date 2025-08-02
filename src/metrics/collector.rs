use anyhow::Result;
use dashmap::DashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::{debug, error, info};

use crate::config::MetricsConfig;
use crate::openstack::Client;
use crate::openstack::services::{ServerMetrics, NetworkMetrics, StorageMetrics};
use super::kafka_producer::KafkaProducer;

pub struct MetricsCollector {
    config: MetricsConfig,
    openstack_client: Arc<Client>,
    kafka_producer: KafkaProducer,
    active_resources: Arc<DashMap<String, ResourceInfo>>,
}

#[derive(Debug, Clone)]
pub struct ResourceInfo {
    pub resource_type: String,
    pub last_collected: chrono::DateTime<chrono::Utc>,
    pub collection_interval: Duration,
}

impl MetricsCollector {
    pub async fn new(
        config: &MetricsConfig,
        openstack_client: Arc<Client>,
    ) -> Result<Self> {
        let kafka_producer = KafkaProducer::new(&config.kafka_config).await?;
        
        Ok(Self {
            config: config.clone(),
            openstack_client,
            kafka_producer,
            active_resources: Arc::new(DashMap::new()),
        })
    }
    
    pub async fn start_collection(&self) -> Result<()> {
        info!("Starting metrics collection service");
        
        // Start resource discovery
        let discovery_handle = tokio::spawn({
            let collector = self.clone();
            async move {
                collector.resource_discovery_loop().await;
            }
        });
        
        // Start metrics collection
        let collection_handle = tokio::spawn({
            let collector = self.clone();
            async move {
                collector.metrics_collection_loop().await;
            }
        });
        
        // Start EDF scheduler for critical metrics
        let edf_handle = tokio::spawn({
            let collector = self.clone();
            async move {
                collector.edf_scheduling_loop().await;
            }
        });
        
        // Wait for all tasks
        tokio::try_join!(discovery_handle, collection_handle, edf_handle)?;
        
        Ok(())
    }
    
    async fn resource_discovery_loop(&self) {
        let mut interval = interval(Duration::from_secs(self.config.discovery_interval_seconds));
        
        loop {
            interval.tick().await;
            
            if let Err(e) = self.discover_resources().await {
                error!("Resource discovery failed: {}", e);
            }
        }
    }
    
    async fn discover_resources(&self) -> Result<()> {
        debug!("Discovering OpenStack resources");
        
        // Discover compute instances
        let servers = self.openstack_client.nova.list_servers().await?;
        for server in servers {
            self.active_resources.insert(
                server.id.clone(),
                ResourceInfo {
                    resource_type: "compute".to_string(),
                    last_collected: chrono::Utc::now(),
                    collection_interval: Duration::from_secs(self.config.compute_interval_seconds),
                }
            );
        }
        
        debug!("Discovered {} compute resources", self.active_resources.len());
        Ok(())
    }
    
    async fn metrics_collection_loop(&self) {
        let mut interval = interval(Duration::from_millis(100)); // High frequency for real-time
        
        loop {
            interval.tick().await;
            
            if let Err(e) = self.collect_all_metrics().await {
                error!("Metrics collection failed: {}", e);
            }
        }
    }
    
    async fn collect_all_metrics(&self) -> Result<()> {
        let now = chrono::Utc::now();
        let mut collection_tasks = Vec::new();
        
        // Collect metrics for resources that need updating
        for entry in self.active_resources.iter() {
            let resource_id = entry.key().clone();
            let resource_info = entry.value().clone();
            
            if now.signed_duration_since(resource_info.last_collected).num_seconds() 
                >= resource_info.collection_interval.as_secs() as i64 {
                
                let client = self.openstack_client.clone();
                let producer = self.kafka_producer.clone();
                
                let task = tokio::spawn(async move {
                    match resource_info.resource_type.as_str() {
                        "compute" => {
                            if let Ok(metrics) = client.nova.get_server_metrics(&resource_id).await {
                                let _ = producer.send_server_metrics(&metrics).await;
                            }
                        },
                        "network" => {
                            if let Ok(metrics) = client.neutron.get_network_metrics().await {
                                for metric in metrics {
                                    let _ = producer.send_network_metrics(&metric).await;
                                }
                            }
                        },
                        "storage" => {
                            if let Ok(metrics) = client.cinder.get_storage_metrics().await {
                                for metric in metrics {
                                    let _ = producer.send_storage_metrics(&metric).await;
                                }
                            }
                        },
                        _ => {}
                    }
                });
                
                collection_tasks.push(task);
            }
        }
        
        // Wait for all collection tasks to complete
        for task in collection_tasks {
            let _ = task.await;
        }
        
        Ok(())
    }
    
    async fn edf_scheduling_loop(&self) {
        let mut interval = interval(Duration::from_millis(10)); // EDF requires high frequency
        
        loop {
            interval.tick().await;
            
            // Implement Earliest Deadline First scheduling for critical metrics
            self.process_edf_queue().await;
        }
    }
    
    async fn process_edf_queue(&self) {
        // Priority queue implementation for EDF scheduling
        // This would prioritize metrics collection based on SLA requirements
        // and deadline constraints
        
        // Mock implementation - in reality this would:
        // 1. Sort resources by deadline urgency
        // 2. Process critical SLA-bound resources first
        // 3. Ensure real-time constraints are met
        
        debug!("Processing EDF scheduling queue");
    }
}

impl Clone for MetricsCollector {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            openstack_client: self.openstack_client.clone(),
            kafka_producer: self.kafka_producer.clone(),
            active_resources: self.active_resources.clone(),
        }
    }
}
