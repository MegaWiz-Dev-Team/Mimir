//! Relational dataset registry (MariaDB) — the catalog side of mimir-lab.
//!
//! Schema: `migrations/0001_init_analytics.sql`. Uses **runtime** sqlx queries
//! (not the compile-time `query!` macro) so the crate builds without a live
//! `DATABASE_URL`. Dataset *data* lives in DuckDB/Parquet/MinIO; this tracks
//! metadata + the PII gate state per ADR-024.

use crate::error::Result;
use crate::pii::PiiStatus;
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;

/// A registered dataset row (lean projection — timestamps omitted).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct Dataset {
    pub id: String,
    pub tenant_id: String,
    pub name: String,
    pub source_type: String,
    pub schema_json: Option<String>,
    pub storage_uri: Option<String>,
    pub row_count: i64,
    pub pii_status: String,
    pub pii_categories: Option<String>,
    pub created_by: Option<String>,
}

/// Parameters to register a new dataset (id + pii_status are assigned).
#[derive(Debug, Clone)]
pub struct NewDataset {
    pub tenant_id: String,
    pub name: String,
    /// One of the `source_type` enum values: `upload` | `cross_tenant` | `external`.
    pub source_type: String,
    pub schema_json: Option<String>,
    pub storage_uri: Option<String>,
    pub row_count: i64,
    pub created_by: Option<String>,
}

const COLS: &str = "id, tenant_id, name, source_type, schema_json, storage_uri, row_count, pii_status, pii_categories, created_by";

pub struct Registry {
    pool: MySqlPool,
}

impl Registry {
    pub async fn connect(url: &str) -> Result<Self> {
        Ok(Self {
            pool: MySqlPool::connect(url).await?,
        })
    }

    pub fn from_pool(pool: MySqlPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &MySqlPool {
        &self.pool
    }

    /// Register a dataset; returns the generated id. Starts in `pending` PII state.
    pub async fn register_dataset(&self, d: NewDataset) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO datasets \
             (id, tenant_id, name, source_type, schema_json, storage_uri, row_count, pii_status, created_by) \
             VALUES (?, ?, ?, ?, ?, ?, ?, 'pending', ?)",
        )
        .bind(&id)
        .bind(&d.tenant_id)
        .bind(&d.name)
        .bind(&d.source_type)
        .bind(&d.schema_json)
        .bind(&d.storage_uri)
        .bind(d.row_count)
        .bind(&d.created_by)
        .execute(&self.pool)
        .await?;
        Ok(id)
    }

    /// List datasets for a tenant, ordered by name.
    pub async fn list_datasets(&self, tenant_id: &str) -> Result<Vec<Dataset>> {
        let sql = format!("SELECT {COLS} FROM datasets WHERE tenant_id = ? ORDER BY name");
        Ok(sqlx::query_as::<_, Dataset>(&sql)
            .bind(tenant_id)
            .fetch_all(&self.pool)
            .await?)
    }

    /// Fetch one dataset (profile) by id.
    pub async fn get_dataset(&self, id: &str) -> Result<Option<Dataset>> {
        let sql = format!("SELECT {COLS} FROM datasets WHERE id = ?");
        Ok(sqlx::query_as::<_, Dataset>(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?)
    }

    /// Update the PII gate state (and categories when flagged).
    pub async fn update_pii_status(&self, id: &str, status: &PiiStatus) -> Result<()> {
        let (s, cats) = match status {
            PiiStatus::Pending => ("pending", None),
            PiiStatus::Clean => ("clean", None),
            PiiStatus::Flagged { categories } => {
                ("flagged", Some(serde_json::to_string(categories)?))
            }
        };
        sqlx::query("UPDATE datasets SET pii_status = ?, pii_categories = ? WHERE id = ?")
            .bind(s)
            .bind(cats)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Record an immutable version snapshot; returns the version row id.
    pub async fn record_version(
        &self,
        dataset_id: &str,
        version: i32,
        storage_uri: Option<&str>,
        checksum: Option<&str>,
    ) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO dataset_versions (id, dataset_id, version, storage_uri, checksum) \
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(dataset_id)
        .bind(version)
        .bind(storage_uri)
        .bind(checksum)
        .execute(&self.pool)
        .await?;
        Ok(id)
    }

    /// Delete a dataset (and its versions) by id.
    pub async fn delete_dataset(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM dataset_versions WHERE dataset_id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM datasets WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
