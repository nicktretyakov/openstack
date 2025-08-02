use anyhow::Result;
use chrono::{DateTime, Utc, Duration};
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::config::OpenStackConfig;
use crate::error::OpenStackError;

#[derive(Debug, Clone)]
pub struct AuthToken {
    pub token: String,
    pub expires_at: DateTime<Utc>,
    pub project_id: String,
    pub user_id: String,
}

impl AuthToken {
    pub fn is_expired(&self) -> bool {
        Utc::now() + Duration::minutes(5) > self.expires_at
    }
}

#[derive(Serialize)]
struct AuthRequest {
    auth: AuthPayload,
}

#[derive(Serialize)]
struct AuthPayload {
    identity: Identity,
    scope: Scope,
}

#[derive(Serialize)]
struct Identity {
    methods: Vec<String>,
    password: PasswordAuth,
}

#[derive(Serialize)]
struct PasswordAuth {
    user: UserAuth,
}

#[derive(Serialize)]
struct UserAuth {
    name: String,
    domain: Domain,
    password: String,
}

#[derive(Serialize)]
struct Domain {
    name: String,
}

#[derive(Serialize)]
struct Scope {
    project: Project,
}

#[derive(Serialize)]
struct Project {
    name: String,
    domain: Domain,
}

#[derive(Deserialize)]
struct AuthResponse {
    token: TokenInfo,
}

#[derive(Deserialize)]
struct TokenInfo {
    expires_at: String,
    project: ProjectInfo,
    user: UserInfo,
}

#[derive(Deserialize)]
struct ProjectInfo {
    id: String,
}

#[derive(Deserialize)]
struct UserInfo {
    id: String,
}

pub struct AuthManager {
    config: OpenStackConfig,
    http_client: HttpClient,
    current_token: Option<AuthToken>,
}

impl AuthManager {
    pub async fn new(config: OpenStackConfig, http_client: HttpClient) -> Result<Self> {
        let mut manager = Self {
            config,
            http_client,
            current_token: None,
        };
        
        // Get initial token
        manager.refresh_token().await?;
        
        Ok(manager)
    }
    
    pub async fn get_token(&self) -> Result<&AuthToken> {
        if let Some(ref token) = self.current_token {
            if !token.is_expired() {
                return Ok(token);
            }
        }
        
        // Token is expired or doesn't exist, need to refresh
        // In a real implementation, this would need proper synchronization
        Err(OpenStackError::AuthError("Token expired, refresh needed".to_string()).into())
    }
    
    pub async fn refresh_token(&mut self) -> Result<()> {
        debug!("Refreshing OpenStack authentication token");
        
        let auth_request = AuthRequest {
            auth: AuthPayload {
                identity: Identity {
                    methods: vec!["password".to_string()],
                    password: PasswordAuth {
                        user: UserAuth {
                            name: self.config.username.clone(),
                            domain: Domain {
                                name: self.config.user_domain.clone(),
                            },
                            password: self.config.password.clone(),
                        },
                    },
                },
                scope: Scope {
                    project: Project {
                        name: self.config.project_name.clone(),
                        domain: Domain {
                            name: self.config.project_domain.clone(),
                        },
                    },
                },
            },
        };
        
        let response = self.http_client
            .post(&format!("{}/v3/auth/tokens", self.config.auth_url))
            .json(&auth_request)
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(OpenStackError::AuthError(
                format!("Authentication failed: {}", response.status())
            ).into());
        }
        
        let token_header = response.headers()
            .get("X-Subject-Token")
            .ok_or_else(|| OpenStackError::AuthError("No token in response".to_string()))?
            .to_str()?
            .to_string();
        
        let auth_response: AuthResponse = response.json().await?;
        
        let expires_at = DateTime::parse_from_rfc3339(&auth_response.token.expires_at)?
            .with_timezone(&Utc);
        
        self.current_token = Some(AuthToken {
            token: token_header,
            expires_at,
            project_id: auth_response.token.project.id,
            user_id: auth_response.token.user.id,
        });
        
        debug!("Authentication token refreshed successfully");
        Ok(())
    }
}
