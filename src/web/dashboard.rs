use anyhow::Result;
use axum::{
    extract::{Query, State, WebSocketUpgrade},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::services::ServeDir;
use tracing::{info, warn};

use crate::ml::MLEngine;
use crate::metrics::MetricsCollector;
use crate::scheduler::ResourceScheduler;
use super::websocket::WebSocketHandler;

#[derive(Clone)]
pub struct DashboardServer {
    ml_engine: Arc<MLEngine>,
    metrics_collector: Arc<MetricsCollector>,
    scheduler: Arc<ResourceScheduler>,
    websocket_handler: Arc<WebSocketHandler>,
    dashboard_state: Arc<RwLock<DashboardState>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardState {
    pub active_predictions: HashMap<String, PredictionData>,
    pub system_metrics: SystemMetrics,
    pub alerts: Vec<Alert>,
    pub performance_stats: PerformanceStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionData {
    pub resource_id: String,
    pub resource_type: String,
    pub current_value: f64,
    pub predicted_values: Vec<f64>,
    pub confidence: f64,
    pub trend: String,
    pub last_updated: chrono::DateTime<chrono::Utc>,
    pub model_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub total_resources: u32,
    pub active_predictions: u32,
    pub model_accuracy: f64,
    pub inference_latency_ms: f64,
    pub memory_usage_mb: f64,
    pub cpu_usage_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub id: String,
    pub severity: AlertSeverity,
    pub message: String,
    pub resource_id: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub acknowledged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Critical,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceStats {
    pub predictions_per_second: f64,
    pub model_inference_time_ms: f64,
    pub data_processing_time_ms: f64,
    pub total_predictions_today: u64,
    pub accuracy_trend: Vec<f64>,
}

impl Default for DashboardState {
    fn default() -> Self {
        Self {
            active_predictions: HashMap::new(),
            system_metrics: SystemMetrics {
                total_resources: 0,
                active_predictions: 0,
                model_accuracy: 0.0,
                inference_latency_ms: 0.0,
                memory_usage_mb: 0.0,
                cpu_usage_percent: 0.0,
            },
            alerts: Vec::new(),
            performance_stats: PerformanceStats {
                predictions_per_second: 0.0,
                model_inference_time_ms: 0.0,
                data_processing_time_ms: 0.0,
                total_predictions_today: 0,
                accuracy_trend: Vec::new(),
            },
        }
    }
}

impl DashboardServer {
    pub fn new(
        ml_engine: Arc<MLEngine>,
        metrics_collector: Arc<MetricsCollector>,
        scheduler: Arc<ResourceScheduler>,
    ) -> Self {
        let websocket_handler = Arc::new(WebSocketHandler::new());
        
        Self {
            ml_engine,
            metrics_collector,
            scheduler,
            websocket_handler,
            dashboard_state: Arc::new(RwLock::new(DashboardState::default())),
        }
    }
    
    pub async fn start(&self, port: u16) -> Result<()> {
        info!("Starting ML monitoring dashboard on port {}", port);
        
        // Start background tasks
        let state_updater = self.clone();
        tokio::spawn(async move {
            state_updater.update_dashboard_state_loop().await;
        });
        
        // Create router
        let app = Router::new()
            .route("/", get(serve_dashboard))
            .route("/api/predictions", get(get_predictions))
            .route("/api/metrics", get(get_system_metrics))
            .route("/api/alerts", get(get_alerts))
            .route("/api/alerts/:id/acknowledge", post(acknowledge_alert))
            .route("/api/performance", get(get_performance_stats))
            .route("/ws", get(websocket_handler))
            .nest_service("/static", ServeDir::new("static"))
            .with_state(self.clone());
        
        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
        info!("Dashboard server listening on http://0.0.0.0:{}", port);
        
        axum::serve(listener, app).await?;
        Ok(())
    }
    
    async fn update_dashboard_state_loop(&self) {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
        
        loop {
            interval.tick().await;
            
            if let Err(e) = self.update_dashboard_state().await {
                warn!("Failed to update dashboard state: {}", e);
            }
        }
    }
    
    async fn update_dashboard_state(&self) -> Result<()> {
        let mut state = self.dashboard_state.write().await;
        
        // Update predictions
        self.update_predictions(&mut state).await?;
        
        // Update system metrics
        self.update_system_metrics(&mut state).await?;
        
        // Update alerts
        self.update_alerts(&mut state).await?;
        
        // Update performance stats
        self.update_performance_stats(&mut state).await?;
        
        // Broadcast updates via WebSocket
        let state_json = serde_json::to_string(&*state)?;
        self.websocket_handler.broadcast(state_json).await;
        
        Ok(())
    }
    
    async fn update_predictions(&self, state: &mut DashboardState) -> Result<()> {
        // Mock implementation - in reality would get from ML engine
        let resource_ids = vec!["vm-001", "vm-002", "vm-003", "host-001", "host-002"];
        
        for resource_id in resource_ids {
            let predicted_load = self.ml_engine
                .get_resource_prediction(resource_id)
                .await
                .unwrap_or(0.0);
            
            let prediction_data = PredictionData {
                resource_id: resource_id.to_string(),
                resource_type: if resource_id.starts_with("vm") { "VM" } else { "Host" }.to_string(),
                current_value: 45.0 + rand::random::<f64>() * 30.0,
                predicted_values: self.generate_prediction_series(predicted_load).await,
                confidence: 0.85 + rand::random::<f64>() * 0.1,
                trend: self.determine_trend(predicted_load),
                last_updated: chrono::Utc::now(),
                model_version: "v1.0.1".to_string(),
            };
            
            state.active_predictions.insert(resource_id.to_string(), prediction_data);
        }
        
        Ok(())
    }
    
    async fn generate_prediction_series(&self, base_value: f64) -> Vec<f64> {
        (0..24).map(|i| {
            let trend = 0.1 * i as f64;
            let seasonal = 5.0 * (2.0 * std::f64::consts::PI * i as f64 / 24.0).sin();
            let noise = (rand::random::<f64>() - 0.5) * 2.0;
            (base_value + trend + seasonal + noise).max(0.0).min(100.0)
        }).collect()
    }
    
    fn determine_trend(&self, value: f64) -> String {
        if value > 70.0 {
            "Increasing".to_string()
        } else if value < 30.0 {
            "Decreasing".to_string()
        } else {
            "Stable".to_string()
        }
    }
    
    async fn update_system_metrics(&self, state: &mut DashboardState) -> Result<()> {
        state.system_metrics = SystemMetrics {
            total_resources: state.active_predictions.len() as u32,
            active_predictions: state.active_predictions.len() as u32,
            model_accuracy: 0.87 + rand::random::<f64>() * 0.1,
            inference_latency_ms: 15.0 + rand::random::<f64>() * 10.0,
            memory_usage_mb: 512.0 + rand::random::<f64>() * 100.0,
            cpu_usage_percent: 25.0 + rand::random::<f64>() * 20.0,
        };
        
        Ok(())
    }
    
    async fn update_alerts(&self, state: &mut DashboardState) -> Result<()> {
        // Generate sample alerts based on predictions
        for (resource_id, prediction) in &state.active_predictions {
            if prediction.current_value > 90.0 {
                let alert = Alert {
                    id: format!("alert-{}-{}", resource_id, chrono::Utc::now().timestamp()),
                    severity: AlertSeverity::Critical,
                    message: format!("High resource utilization detected on {}: {:.1}%", 
                                   resource_id, prediction.current_value),
                    resource_id: Some(resource_id.clone()),
                    timestamp: chrono::Utc::now(),
                    acknowledged: false,
                };
                
                // Only add if not already present
                if !state.alerts.iter().any(|a| a.resource_id.as_ref() == Some(resource_id) && 
                                           matches!(a.severity, AlertSeverity::Critical)) {
                    state.alerts.push(alert);
                }
            }
            
            if prediction.confidence < 0.7 {
                let alert = Alert {
                    id: format!("alert-conf-{}-{}", resource_id, chrono::Utc::now().timestamp()),
                    severity: AlertSeverity::Warning,
                    message: format!("Low prediction confidence for {}: {:.1}%", 
                                   resource_id, prediction.confidence * 100.0),
                    resource_id: Some(resource_id.clone()),
                    timestamp: chrono::Utc::now(),
                    acknowledged: false,
                };
                
                if !state.alerts.iter().any(|a| a.resource_id.as_ref() == Some(resource_id) && 
                                           matches!(a.severity, AlertSeverity::Warning)) {
                    state.alerts.push(alert);
                }
            }
        }
        
        // Remove old alerts (older than 1 hour)
        let cutoff = chrono::Utc::now() - chrono::Duration::hours(1);
        state.alerts.retain(|alert| alert.timestamp > cutoff);
        
        Ok(())
    }
    
    async fn update_performance_stats(&self, state: &mut DashboardState) -> Result<()> {
        state.performance_stats.predictions_per_second = 
            state.active_predictions.len() as f64 / 60.0; // Assuming 1-minute intervals
        
        state.performance_stats.model_inference_time_ms = 
            12.0 + rand::random::<f64>() * 8.0;
        
        state.performance_stats.data_processing_time_ms = 
            5.0 + rand::random::<f64>() * 3.0;
        
        state.performance_stats.total_predictions_today += 
            state.active_predictions.len() as u64;
        
        // Update accuracy trend (keep last 100 points)
        let new_accuracy = 0.85 + rand::random::<f64>() * 0.1;
        state.performance_stats.accuracy_trend.push(new_accuracy);
        if state.performance_stats.accuracy_trend.len() > 100 {
            state.performance_stats.accuracy_trend.remove(0);
        }
        
        Ok(())
    }
}

// API Handlers
async fn serve_dashboard() -> Html<&'static str> {
    Html(include_str!("../../static/dashboard.html"))
}

async fn get_predictions(State(server): State<DashboardServer>) -> impl IntoResponse {
    let state = server.dashboard_state.read().await;
    Json(state.active_predictions.clone())
}

async fn get_system_metrics(State(server): State<DashboardServer>) -> impl IntoResponse {
    let state = server.dashboard_state.read().await;
    Json(state.system_metrics.clone())
}

async fn get_alerts(State(server): State<DashboardServer>) -> impl IntoResponse {
    let state = server.dashboard_state.read().await;
    Json(state.alerts.clone())
}

async fn get_performance_stats(State(server): State<DashboardServer>) -> impl IntoResponse {
    let state = server.dashboard_state.read().await;
    Json(state.performance_stats.clone())
}

#[derive(Deserialize)]
struct AcknowledgeParams {
    id: String,
}

async fn acknowledge_alert(
    State(server): State<DashboardServer>,
    Query(params): Query<AcknowledgeParams>,
) -> impl IntoResponse {
    let mut state = server.dashboard_state.write().await;
    
    if let Some(alert) = state.alerts.iter_mut().find(|a| a.id == params.id) {
        alert.acknowledged = true;
        (StatusCode::OK, "Alert acknowledged")
    } else {
        (StatusCode::NOT_FOUND, "Alert not found")
    }
}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(server): State<DashboardServer>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        server.websocket_handler.handle_connection(socket).await;
    })
}
