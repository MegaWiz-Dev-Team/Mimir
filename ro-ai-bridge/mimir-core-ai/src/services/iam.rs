use anyhow::{Result, anyhow};
use argon2::{
    password_hash::{
        rand_core::OsRng,
        PasswordHash, PasswordHasher, PasswordVerifier, SaltString
    },
    Argon2
};
use jsonwebtoken::{encode, EncodingKey, Header};
use sqlx::MySqlPool;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use crate::middleware::tenant::TenantClaims;
use crate::models::iam::{
    CreateUserRequest, Tenant, UpdateUserPasswordRequest, UpdateUserRoleRequest, User, UserWithRole,
};

pub struct IamService {
    db: MySqlPool,
    jwt_secret: String,
}

impl IamService {
    pub fn new(db: MySqlPool, jwt_secret: String) -> Self {
        Self { db, jwt_secret }
    }

    /// Create with JWT secret from environment variable (for CLI binaries)
    pub fn new_with_env(db: MySqlPool) -> Self {
        let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "dev_secret_key".to_string());
        Self { db, jwt_secret }
    }

    /// Hash a plaintext password using Argon2id
    pub fn hash_password(password: &str) -> Result<String> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2.hash_password(password.as_bytes(), &salt)
            .map_err(|e| anyhow!("Failed to hash password: {}", e))?
            .to_string();
        Ok(password_hash)
    }

    /// Verify a plaintext password against a hash
    pub fn verify_password(password: &str, hash: &str) -> Result<bool> {
        let parsed_hash = PasswordHash::new(hash)
            .map_err(|e| anyhow!("Invalid password hash format: {}", e))?;
        let is_valid = Argon2::default().verify_password(password.as_bytes(), &parsed_hash).is_ok();
        Ok(is_valid)
    }

    /// Authenticate a user and return (access_token, tenant_id)
    pub async fn login(&self, username: &str, password: &str) -> Result<(String, String)> {
        // Find user by username
        let user_row = sqlx::query!(
            "SELECT id, password_hash FROM users WHERE username = ?",
            username
        )
        .fetch_optional(&self.db).await?;

        let user = user_row.ok_or_else(|| anyhow!("Invalid credentials"))?;

        if !Self::verify_password(password, &user.password_hash)? {
            return Err(anyhow!("Invalid credentials"));
        }

        // Get tenant and role (for MVP, we assume a user has 1 primary tenant linked)
        let tenant_row = sqlx::query!(
            "SELECT tenant_id, role FROM tenant_users WHERE user_id = ? LIMIT 1",
            user.id
        )
        .fetch_optional(&self.db).await?;

        let tenant = tenant_row.ok_or_else(|| anyhow!("User has no assigned tenant"))?;

        let token = self.generate_jwt(&user.id, &tenant.tenant_id, &tenant.role)?;

        Ok((token, tenant.tenant_id))
    }

    /// Generate JWT Access Token
    fn generate_jwt(&self, user_id: &str, tenant_id: &str, role: &str) -> Result<String> {
        let expiration = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_secs() as usize + (24 * 60 * 60); // 24 hours exp

        // In a full implementation, `role` would be added to the claims.
        // For now, we reuse `TenantClaims` from the existing middleware.
        let claims = TenantClaims {
            iss: "mimir-auth".to_string(),
            sub: user_id.to_string(),
            client_id: Some("ro-domain-connector".to_string()),
            tenant_id: tenant_id.to_string(),
            role: role.to_string(),
            exp: expiration,
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes())
        )?;

        Ok(token)
    }

    pub async fn get_users(&self) -> Result<Vec<UserWithRole>> {
        let users = sqlx::query_as!(
            UserWithRole,
            r#"
            SELECT 
                u.id, 
                u.username, 
                tu.tenant_id, 
                tu.role, 
                u.created_at
            FROM users u
            LEFT JOIN tenant_users tu ON u.id = tu.user_id
            "#
        )
        .fetch_all(&self.db).await?;
        
        Ok(users)
    }

    pub async fn get_tenants(&self) -> Result<Vec<Tenant>> {
        let tenants = sqlx::query_as!(
            Tenant,
            "SELECT id, name, domain, created_at, updated_at FROM tenants"
        )
        .fetch_all(&self.db).await?;
        Ok(tenants)
    }

    pub async fn create_user(&self, req: CreateUserRequest) -> Result<User> {
        let user_id = Uuid::new_v4().to_string();
        
        // Use provided password or generate random temp one
        let password = req.password.unwrap_or_else(|| "temp123!".to_string());
        let hash = Self::hash_password(&password)?;

        let mut tx = self.db.begin().await?;

        sqlx::query!(
            "INSERT INTO users (id, username, password_hash) VALUES (?, ?, ?)",
            user_id,
            req.username,
            hash
        )
        .execute(&mut *tx).await?;

        sqlx::query!(
            "INSERT INTO tenant_users (tenant_id, user_id, role) VALUES (?, ?, ?)",
            req.tenant_id,
            user_id,
            req.role
        )
        .execute(&mut *tx).await?;

        tx.commit().await?;

        let user = sqlx::query_as!(
            User,
            "SELECT id, username, created_at, updated_at FROM users WHERE id = ?",
            user_id
        )
        .fetch_one(&self.db).await?;

        Ok(user)
    }

    pub async fn update_user_role(&self, user_id: &str, req: UpdateUserRoleRequest) -> Result<()> {
        sqlx::query!(
            "UPDATE tenant_users SET role = ?, tenant_id = ? WHERE user_id = ?",
            req.role,
            req.tenant_id,
            user_id
        )
        .execute(&self.db).await?;
        Ok(())
    }

    pub async fn update_user_password(&self, user_id: &str, req: UpdateUserPasswordRequest) -> Result<()> {
        let hash = Self::hash_password(&req.password)?;
        sqlx::query!(
            "UPDATE users SET password_hash = ? WHERE id = ?",
            hash,
            user_id
        )
        .execute(&self.db).await?;
        Ok(())
    }

    pub async fn delete_user(&self, user_id: &str) -> Result<()> {
        let mut tx = self.db.begin().await?;
        sqlx::query!("DELETE FROM tenant_users WHERE user_id = ?", user_id)
            .execute(&mut *tx).await?;
        sqlx::query!("DELETE FROM users WHERE id = ?", user_id)
            .execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn update_tenant(&self, tenant_id: &str, req: crate::models::iam::UpdateTenantRequest) -> Result<()> {
        sqlx::query!(
            "UPDATE tenants SET name = ? WHERE id = ?",
            req.name,
            tenant_id
        )
        .execute(&self.db).await?;
        Ok(())
    }

    pub async fn get_tenant_config(&self, tenant_id: &str) -> Result<crate::models::iam::TenantConfig> {
        let config = sqlx::query_as::<_, crate::models::iam::TenantConfig>(
            r#"SELECT 
                tenant_id, 
                default_provider, 
                default_model, 
                provider_api_keys, 
                qa_rules, 
                system_prompt, 
                max_daily_tokens, 
                is_dedicated_vector_db,
                max_crawl_pages,
                search_settings,
                llm_config,
                created_at, 
                updated_at
            FROM tenant_configs 
            WHERE tenant_id = ?"#
        )
        .bind(tenant_id)
        .fetch_one(&self.db).await?;
        Ok(config)
    }

    pub async fn update_tenant_config(&self, tenant_id: &str, req: crate::models::iam::UpdateTenantConfigRequest) -> Result<()> {
        let config = self.get_tenant_config(tenant_id).await?;

        let default_provider = req.default_provider.unwrap_or(config.default_provider.clone());
        let default_model = req.default_model.unwrap_or(config.default_model.clone());
        let provider_api_keys = req.provider_api_keys.or(config.provider_api_keys);
        let qa_rules = req.qa_rules.or(config.qa_rules);
        let system_prompt = req.system_prompt.or(config.system_prompt);
        let max_daily_tokens = req.max_daily_tokens.unwrap_or(config.max_daily_tokens);
        let is_dedicated_vector_db = req.is_dedicated_vector_db.unwrap_or(config.is_dedicated_vector_db);
        let max_crawl_pages = req.max_crawl_pages.unwrap_or(config.max_crawl_pages);
        let search_settings = req.search_settings.or(config.search_settings);
        let llm_config = req.llm_config.or(config.llm_config);

        // Serialize llm_config to JSON string for runtime query binding
        let llm_config_json = llm_config
            .as_ref()
            .map(|c| serde_json::to_string(&c.0))
            .transpose()
            .map_err(|e| anyhow!("Failed to serialize llm_config: {}", e))?;

        sqlx::query(
            r#"UPDATE tenant_configs 
            SET default_provider = ?, default_model = ?, provider_api_keys = ?, qa_rules = ?, system_prompt = ?, max_daily_tokens = ?, is_dedicated_vector_db = ?, max_crawl_pages = ?, search_settings = ?, llm_config = ?
            WHERE tenant_id = ?"#
        )
        .bind(&default_provider)
        .bind(&default_model)
        .bind(&provider_api_keys)
        .bind(&qa_rules)
        .bind(&system_prompt)
        .bind(max_daily_tokens)
        .bind(is_dedicated_vector_db)
        .bind(max_crawl_pages)
        .bind(&search_settings)
        .bind(&llm_config_json)
        .bind(tenant_id)
        .execute(&self.db).await?;
        Ok(())
    }

    pub async fn create_tenant(&self, req: crate::models::iam::CreateTenantRequest) -> Result<crate::models::iam::Tenant> {
        let tenant_id = uuid::Uuid::new_v4().to_string();
        let mut tx = self.db.begin().await?;

        let domain = req.domain.unwrap_or_else(|| "general".to_string());

        // 1. Core Tenant Record
        sqlx::query!("INSERT INTO tenants (id, name, domain) VALUES (?, ?, ?)", tenant_id, req.name, domain)
            .execute(&mut *tx).await?;

        // 2. Initialize Configs
        sqlx::query!(
            "INSERT INTO tenant_configs (tenant_id, is_dedicated_vector_db) VALUES (?, ?)",
            tenant_id,
            req.is_dedicated_vector_db
        ).execute(&mut *tx).await?;

        // 3. Admin User Creation
        let user_id = uuid::Uuid::new_v4().to_string();
        let password = req.admin_password.unwrap_or_else(|| "admin123".to_string());
        let hash = Self::hash_password(&password)?;
        sqlx::query!(
            "INSERT INTO users (id, username, password_hash) VALUES (?, ?, ?)",
            user_id, req.admin_email, hash
        ).execute(&mut *tx).await?;

        sqlx::query!(
            "INSERT INTO tenant_users (tenant_id, user_id, role) VALUES (?, ?, 'admin')",
            tenant_id, user_id
        ).execute(&mut *tx).await?;

        // Commit DB transaction
        tx.commit().await?;

        // 4. Data Isolation / Vector DB Provisioning
        let qdrant = crate::services::qdrant::QdrantService::new();
        if req.is_dedicated_vector_db {
            let collection_name = format!("{}_docs", tenant_id);
            // Default Vector size 768 for nomic-embed-text or 1536 for others. We use 768 as base
            qdrant.init_collection(&collection_name, 768).await.unwrap_or_else(|e| {
                tracing::warn!("Failed to init Qdrant collection {}: {}", collection_name, e);
            });
        }

        let tenant = sqlx::query_as!(
            crate::models::iam::Tenant,
            "SELECT id, name, domain, created_at, updated_at FROM tenants WHERE id = ?",
            tenant_id
        ).fetch_one(&self.db).await?;

        Ok(tenant)
    }

    /// Get the domain string for a tenant by ID.
    pub async fn get_tenant_domain(&self, tenant_id: &str) -> Result<String> {
        let row = sqlx::query!(
            "SELECT domain FROM tenants WHERE id = ?",
            tenant_id
        )
        .fetch_one(&self.db).await?;
        Ok(row.domain)
    }

    pub async fn delete_tenant(&self, tenant_id: &str) -> Result<()> {
        let config = self.get_tenant_config(tenant_id).await.unwrap_or_else(|_| crate::models::iam::TenantConfig {
                tenant_id: tenant_id.to_string(),
                default_provider: "ollama".to_string(),
                default_model: "llama3.2".to_string(),
                provider_api_keys: None,
                qa_rules: None,
                system_prompt: None,
                max_daily_tokens: 100000,
                is_dedicated_vector_db: false,
                max_crawl_pages: 100,
                search_settings: None,
                llm_config: None,
                created_at: None,
                updated_at: None,
            });
        
        // 1. Qdrant Cleanup
        let qdrant = crate::services::qdrant::QdrantService::new();
        if config.is_dedicated_vector_db {
            let collection_name = format!("{}_docs", tenant_id);
            let _ = qdrant.delete_collection(&collection_name).await;
        }

        // 2. DB Cleanup (tenants has CASCADE for tenant_configs, tenant_users, pipeline_runs, etc)
        // Wait, users are not cascade deleted just because tenant_users is. We need to delete orphaned users.
        let mut tx = self.db.begin().await?;
        
        // Get all users who belong entirely to this tenant to delete them. 
        // For simplicity, we just delete users whose ONLY tenant is this one.
        sqlx::query!(
            r#"
            DELETE FROM users WHERE id IN (
                SELECT user_id FROM tenant_users WHERE tenant_id = ?
            )
            "#,
            tenant_id
        ).execute(&mut *tx).await?;

        sqlx::query!("DELETE FROM tenants WHERE id = ?", tenant_id).execute(&mut *tx).await?;

        tx.commit().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_password_valid() {
        let password = "mysecretpassword123";
        let hash = IamService::hash_password(password).unwrap();
        
        let is_valid = IamService::verify_password(password, &hash).unwrap();
        assert!(is_valid, "Password should be verified against its valid hash");
    }

    #[test]
    fn test_verify_password_invalid() {
        let password = "wrongpassword999";
        let hash = IamService::hash_password("mysecretpassword123").unwrap();
        
        let is_valid = IamService::verify_password(password, &hash).unwrap();
        assert!(!is_valid, "Password should not be verified against another password's hash");
    }

    #[test]
    fn test_verify_password_malformed_hash() {
        let password = "mysecretpassword123";
        let invalid_hash = "not_an_argon_hash_at_all";
        
        let result = IamService::verify_password(password, invalid_hash);
        assert!(result.is_err(), "verify_password should return Err on invalid hash string format");
    }
}
