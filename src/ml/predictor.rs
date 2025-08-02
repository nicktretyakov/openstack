use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

use super::models::{LSTMModel, TimeSeriesData};

pub struct LoadPredictor {
    lstm_model: Arc<RwLock<LSTMModel>>,
    historical_data: Arc<RwLock<HashMap<String, TimeSeriesData>>>,
}

#[derive(Debug, Clone)]
pub struct LoadPrediction {
    pub resource_id: String,
    pub predicted_load: f64,
    pub confidence: f64,
    pub prediction_horizon_minutes: u32,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl LoadPredictor {
    pub fn new(lstm_model: Arc<RwLock<LSTMModel>>) -> Self {
        Self {
            lstm_model,
            historical_data: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    pub async fn predict_load_next_hour(&self) -> Result<Vec<LoadPrediction>> {
        debug!("Predicting load for next hour");
        
        let mut predictions = Vec::new();
        let historical_data = self.historical_data.read().await;
        
        for (resource_id, time_series) in historical_data.iter() {
            if let Some(recent_data) = time_series.get_recent_window(24) {
                let model = self.lstm_model.read().await;
                
                // Create input data for LSTM
                let input_data = TimeSeriesData {
                    timestamps: vec![chrono::Utc::now()], // Simplified
                    values: recent_data,
                    resource_id: resource_id.clone(),
                    metric_type: "cpu_utilization".to_string(),
                };
                
                if let Ok(prediction_values) = model.predict(&input_data) {
                    // Take the first prediction (next hour)
                    if let Some(&predicted_load) = prediction_values.first() {
                        predictions.push(LoadPrediction {
                            resource_id: resource_id.clone(),
                            predicted_load,
                            confidence: self.calculate_confidence(&recent_data),
                            prediction_horizon_minutes: 60,
                            timestamp: chrono::Utc::now(),
                        });
                    }
                }
            }
        }
        
        Ok(predictions)
    }
    
    pub async fn predict_resource_load(&self, resource_id: &str) -> Result<f64> {
        let historical_data = self.historical_data.read().await;
        
        if let Some(time_series) = historical_data.get(resource_id) {
            if let Some(recent_data) = time_series.get_recent_window(24) {
                let model = self.lstm_model.read().await;
                
                let input_data = TimeSeriesData {
                    timestamps: vec![chrono::Utc::now()],
                    values: recent_data,
                    resource_id: resource_id.to_string(),
                    metric_type: "cpu_utilization".to_string(),
                };
                
                let predictions = model.predict(&input_data)?;
                return Ok(predictions.first().copied().unwrap_or(0.0));
            }
        }
        
        Ok(0.0) // Default prediction if no data available
    }
    
    pub async fn update_historical_data(&self, resource_id: String, value: f64) {
        let mut historical_data = self.historical_data.write().await;
        
        let time_series = historical_data
            .entry(resource_id.clone())
            .or_insert_with(|| TimeSeriesData::new(resource_id, "cpu_utilization".to_string()));
        
        time_series.add_point(chrono::Utc::now(), value);
    }
    
    fn calculate_confidence(&self, recent_data: &[f64]) -> f64 {
        // Simple confidence calculation based on data variance
        if recent_data.len() < 2 {
            return 0.5;
        }
        
        let mean = recent_data.iter().sum::<f64>() / recent_data.len() as f64;
        let variance = recent_data.iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / recent_data.len() as f64;
        
        // Higher variance = lower confidence
        (1.0 / (1.0 + variance)).max(0.1).min(0.95)
    }
}
