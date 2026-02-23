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
    pub fn new(db: MySqlPool) -> Self {
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
            .as_secs() as usize + (15 * 60); // 15 minutes exp

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
            "SELECT id, name, created_at, updated_at FROM tenants"
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
}
