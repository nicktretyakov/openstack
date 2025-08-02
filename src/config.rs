use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub openstack: OpenStackConfig,
    pub metrics: MetricsConfig,
    pub ml: MLConfig,
    pub scheduler: SchedulerConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OpenStackConfig {
    pub auth_url: String,
    pub username: String,
    pub password: String,
    pub project_name: String,
    pub project_domain: String,
    pub user_domain: String,
    pub region_name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MetricsConfig {
    pub discovery_interval_seconds: u64,
    pub compute_interval_seconds: u64,
    pub network_interval_seconds: u64,
    pub storage_interval_seconds: u64,
    pub kafka_config: KafkaConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KafkaConfig {
    pub brokers: String,
    pub compute_topic: String,
    pub network_topic: String,
    pub storage_topic: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MLConfig {
    pub model_path: String,
    pub inference_interval_seconds: u64,
    pub retrain_threshold: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SchedulerConfig {
    pub scheduling_interval_seconds: u64,
    pub high_load_threshold: f64,
    pub low_load_threshold: f64,
    pub sla_check_interval_seconds: u64,
}

impl Config {
    pub fn from_file(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
}
