//! SSO (Single Sign-On) OIDC Configuration and Token Exchange Service

use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use sqlx::MySqlPool;

use crate::services::iam::IamService;

#[derive(Serialize)]
pub struct SsoConfigResponse {
    pub issuer: String,
    pub client_id: String,
    pub redirect_uri: String,
}

#[derive(Deserialize)]
pub struct TokenExchangeRequest {
    pub code: String,
    pub code_verifier: Option<String>,
    pub redirect_uri: Option<String>,
}

#[derive(Serialize)]
pub struct TokenExchangeResponse {
    pub access_token: String,
    pub id_token: Option<String>,
    pub refresh_token: Option<String>,
    pub expires_in: Option<u64>,
    pub token_type: Option<String>,
    pub user_role: String,
    pub user_name: String,
    pub tenant_id: String,
}

#[derive(Deserialize)]
struct ZitadelTokenResponse {
    access_token: String,
    id_token: Option<String>,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
    token_type: Option<String>,
}

pub struct SsoService {
    http_client: Client,
    db: MySqlPool,
    jwt_secret: String,
}

impl SsoService {
    pub fn new(db: MySqlPool, jwt_secret: String) -> Self {
        Self {
            http_client: Client::builder()
                .danger_accept_invalid_certs(true)
                .build()
                .unwrap_or_else(|_| Client::new()),
            db,
            jwt_secret,
        }
    }

    pub fn get_sso_config() -> SsoConfigResponse {
        let issuer = env::var("YGGDRASIL_ISSUER").unwrap_or_else(|_| "http://localhost:8085".to_string());
        let client_id = env::var("YGGDRASIL_CLIENT_ID").unwrap_or_else(|_| "".to_string());
        let redirect_uri = env::var("YGGDRASIL_REDIRECT_URI").unwrap_or_else(|_| "http://localhost:3001/login/callback".to_string());

        SsoConfigResponse {
            issuer,
            client_id,
            redirect_uri,
        }
    }

    fn get_sso_secret() -> String {
        env::var("YGGDRASIL_CLIENT_SECRET").unwrap_or_else(|_| "".to_string())
    }

    pub async fn exchange_code(&self, req: TokenExchangeRequest) -> Result<TokenExchangeResponse> {
        let config = Self::get_sso_config();
        let secret = Self::get_sso_secret();

        if config.client_id.is_empty() || secret.is_empty() {
            return Err(anyhow!("SSO configuration is incomplete in Vault (Missing Client ID or Secret)"));
        }

        let token_url = format!("{}/oauth/v2/token", config.issuer);
        let redirect_uri = req.redirect_uri.unwrap_or(config.redirect_uri);

        let mut form_data = vec![
            ("grant_type", "authorization_code".to_string()),
            ("code", req.code),
            ("redirect_uri", redirect_uri),
            ("client_id", config.client_id.clone()),
            ("client_secret", secret),
        ];

        if let Some(verifier) = req.code_verifier {
            form_data.push(("code_verifier", verifier));
        }

        tracing::info!("[SSO] Token exchange: url={} client_id={}", token_url, config.client_id);

        let res = self.http_client.post(&token_url)
            .form(&form_data)
            .send()
            .await?;

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            tracing::error!("[SSO] Token exchange failed: status={} body={}", status, body);
            return Err(anyhow!("OIDC Token exchange failed: {}", body));
        }

        let tokens: ZitadelTokenResponse = res.json().await?;

        // Extract UserInfo to map Mimir User
        let userinfo_url = format!("{}/oidc/v1/userinfo", config.issuer);
        let userinfo_res = self.http_client.get(&userinfo_url)
            .bearer_auth(&tokens.access_token)
            .send()
            .await?;

        if !userinfo_res.status().is_success() {
            return Err(anyhow!("Failed to fetch userinfo from OIDC provider"));
        }

        let userinfo: serde_json::Value = userinfo_res.json().await?;
        
        // Resolve Identity
        let user_name = userinfo.get("name")
            .or_else(|| userinfo.get("preferred_username"))
            .or_else(|| userinfo.get("email"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let user_login = userinfo.get("preferred_username")
            .or_else(|| userinfo.get("email"))
            .and_then(|v| v.as_str())
            .unwrap_or(&user_name)
            .to_string();

        let mut user_role = "viewer".to_string();
        if let Some(roles) = userinfo.get("urn:zitadel:iam:org:project:roles") {
            if let Some(obj) = roles.as_object() {
                if obj.contains_key("SuperAdmin") {
                    user_role = "SuperAdmin".to_string();
                } else if obj.contains_key("admin") {
                    user_role = "admin".to_string();
                }
            }
        }

        tracing::info!("[SSO] Userinfo resolved login={} name={} role={}", user_login, user_name, user_role);

        // Perform internal JIT SSO authentication
        let iam_service = IamService::new(self.db.clone(), self.jwt_secret.clone());
        let (mimir_token, mimir_tenant, mimir_role) = iam_service.login_sso(&user_login, &user_role).await?;

        tracing::info!("[SSO] Role resolved: zitadel={} → mimir={}", user_role, mimir_role);

        Ok(TokenExchangeResponse {
            access_token: mimir_token,
            id_token: tokens.id_token,
            refresh_token: tokens.refresh_token,
            expires_in: tokens.expires_in,
            token_type: tokens.token_type,
            user_role: mimir_role,
            user_name,
            tenant_id: mimir_tenant,
        })
    }
}
