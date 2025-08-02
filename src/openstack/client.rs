use anyhow::Result;
use reqwest::{Client as HttpClient, header::{HeaderMap, HeaderValue}};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use super::auth::AuthManager;
use super::services::{NovaService, NeutronService, CinderService, TelemetryService};
use crate::config::OpenStackConfig;
use crate::error::OpenStackError;

#[derive(Clone)]
pub struct Client {
    http_client: HttpClient,
    auth_manager: Arc<RwLock<AuthManager>>,
    pub nova: NovaService,
    pub neutron: NeutronService,
    pub cinder: CinderService,
    pub telemetry: TelemetryService,
}

impl Client {
    pub async fn new(config: &OpenStackConfig) -> Result<Self> {
        let http_client = HttpClient::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        
        let auth_manager = Arc::new(RwLock::new(
            AuthManager::new(config.clone(), http_client.clone()).await?
        ));
        
        // Initialize service clients
        let nova = NovaService::new(http_client.clone(), auth_manager.clone());
        let neutron = NeutronService::new(http_client.clone(), auth_manager.clone());
        let cinder = CinderService::new(http_client.clone(), auth_manager.clone());
        let telemetry = TelemetryService::new(http_client.clone(), auth_manager.clone());
        
        info!("OpenStack client initialized successfully");
        
        Ok(Self {
            http_client,
            auth_manager,
            nova,
            neutron,
            cinder,
            telemetry,
        })
    }
    
    pub async fn get_auth_token(&self) -> Result<String> {
        let auth_manager = self.auth_manager.read().await;
        let token = auth_manager.get_token().await?;
        Ok(token.token.clone())
    }
    
    pub async fn make_authenticated_request<T: for<'de> Deserialize<'de>>(
        &self,
        method: reqwest::Method,
        url: &str,
        body: Option<serde_json::Value>,
    ) -> Result<T> {
        let token = self.get_auth_token().await?;
        
        let mut headers = HeaderMap::new();
        headers.insert("X-Auth-Token", HeaderValue::from_str(&token)?);
        headers.insert("Content-Type", HeaderValue::from_static("application/json"));
        
        let mut request = self.http_client
            .request(method, url)
            .headers(headers);
        
        if let Some(body) = body {
            request = request.json(&body);
        }
        
        let response = request.send().await?;
        
        if !response.status().is_success() {
            return Err(OpenStackError::ApiError {
                status: response.status().as_u16(),
                message: response.text().await.unwrap_or_default(),
            }.into());
        }
        
        let result = response.json::<T>().await?;
        Ok(result)
    }
}
