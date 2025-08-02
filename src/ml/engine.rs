use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{debug, error, info};

use crate::config::MLConfig;
use super::models::LSTMModel;
use super::predictor::LoadPredictor;

pub struct MLEngine {
    config: MLConfig,
    lstm_model: Arc<RwLock<LSTMModel>>,
    load_predictor: Arc<LoadPredictor>,
}

impl MLEngine {
    pub async fn new(config: &MLConfig) -> Result<Self> {
        let lstm_model = Arc::new(RwLock::new(
            LSTMModel::load_from_file(&config.model_path).await?
        ));
        
        let load_predictor = Arc::new(
            LoadPredictor::new(lstm_model.clone())
        );
        
        info!("ML Engine initialized successfully");
        
        Ok(Self {
            config: config.clone(),
            lstm_model,
            load_predictor,
        })
    }
    
    pub async fn start_inference_loop(&self) -> Result<()> {
        info!("Starting ML inference loop");
        
        let mut interval = interval(Duration::from_secs(self.config.inference_interval_seconds));
        
        loop {
            interval.tick().await;
            
            if let Err(e) = self.run_inference_cycle().await {
                error!("ML inference cycle failed: {}", e);
            }
        }
    }
    
    async fn run_inference_cycle(&self) -> Result<()> {
        debug!("Running ML inference cycle");
        
        // Get predictions for the next time window
        let predictions = self.load_predictor.predict_load_next_hour().await?;
        
        // Store predictions for scheduler to use
        // In a real implementation, this would write to Redis or similar
        debug!("Generated {} load predictions", predictions.len());
        
        // Check if model needs retraining
        if self.should_retrain_model().await {
            self.retrain_model().await?;
        }
        
        Ok(())
    }
    
    async fn should_retrain_model(&self) -> bool {
        // Implement logic to determine if model needs retraining
        // Based on prediction accuracy, data drift, etc.
        false
    }
    
    async fn retrain_model(&self) -> Result<()> {
        info!("Retraining ML model");
        
        // Hot-swap model without downtime
        let new_model = LSTMModel::retrain(&self.config.model_path).await?;
        
        let mut model_lock = self.lstm_model.write().await;
        *model_lock = new_model;
        
        info!("Model retrained and swapped successfully");
        Ok(())
    }
    
    pub async fn get_resource_prediction(&self, resource_id: &str) -> Result<f64> {
        self.load_predictor.predict_resource_load(resource_id).await
    }
}
