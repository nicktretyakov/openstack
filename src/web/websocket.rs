use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, info};
use uuid::Uuid;

pub struct WebSocketHandler {
    connections: Arc<RwLock<HashMap<String, broadcast::Sender<String>>>>,
    broadcast_tx: broadcast::Sender<String>,
}

impl WebSocketHandler {
    pub fn new() -> Self {
        let (broadcast_tx, _) = broadcast::channel(1000);
        
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            broadcast_tx,
        }
    }
    
    pub async fn handle_connection(&self, socket: WebSocket) {
        let connection_id = Uuid::new_v4().to_string();
        info!("New WebSocket connection: {}", connection_id);
        
        let (tx, mut rx) = broadcast::channel(100);
        
        // Add connection to the map
        {
            let mut connections = self.connections.write().await;
            connections.insert(connection_id.clone(), tx.clone());
        }
        
        // Subscribe to broadcasts
        let mut broadcast_rx = self.broadcast_tx.subscribe();
        
        // Split the socket into sender and receiver
        let (mut sender, mut receiver) = socket.split();
        
        // Handle incoming messages
        let connections_clone = self.connections.clone();
        let connection_id_clone = connection_id.clone();
        
        let recv_task = tokio::spawn(async move {
            while let Some(msg) = receiver.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        debug!("Received message from {}: {}", connection_id_clone, text);
                        // Handle client messages (e.g., subscription requests)
                        if let Err(e) = handle_client_message(&text, &tx).await {
                            error!("Error handling client message: {}", e);
                        }
                    }
                    Ok(Message::Close(_)) => {
                        info!("WebSocket connection closed: {}", connection_id_clone);
                        break;
                    }
                    Err(e) => {
                        error!("WebSocket error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
            
            // Remove connection from map
            let mut connections = connections_clone.write().await;
            connections.remove(&connection_id_clone);
        });
        
        // Handle outgoing messages
        let send_task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    // Broadcast messages
                    Ok(msg) = broadcast_rx.recv() => {
                        if sender.send(Message::Text(msg)).await.is_err() {
                            break;
                        }
                    }
                    // Direct messages to this connection
                    Ok(msg) = rx.recv() => {
                        if sender.send(Message::Text(msg)).await.is_err() {
                            break;
                        }
                    }
                }
            }
        });
        
        // Wait for either task to complete
        tokio::select! {
            _ = recv_task => {},
            _ = send_task => {},
        }
    }
    
    pub async fn broadcast(&self, message: String) {
        if let Err(e) = self.broadcast_tx.send(message) {
            error!("Failed to broadcast message: {}", e);
        }
    }
    
    pub async fn send_to_connection(&self, connection_id: &str, message: String) {
        let connections = self.connections.read().await;
        if let Some(tx) = connections.get(connection_id) {
            if let Err(e) = tx.send(message) {
                error!("Failed to send message to connection {}: {}", connection_id, e);
            }
        }
    }
}

async fn handle_client_message(
    message: &str,
    _tx: &broadcast::Sender<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Parse client messages (e.g., subscription requests, filters)
    if let Ok(request) = serde_json::from_str::<serde_json::Value>(message) {
        match request.get("type").and_then(|t| t.as_str()) {
            Some("subscribe") => {
                debug!("Client subscribed to updates");
                // Handle subscription logic
            }
            Some("filter") => {
                debug!("Client requested filter: {:?}", request.get("filter"));
                // Handle filtering logic
            }
            _ => {
                debug!("Unknown message type: {}", message);
            }
        }
    }
    
    Ok(())
}
