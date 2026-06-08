//! Registry integration tests — env-gated.
//!
//! These need a live MariaDB with the analytics schema applied
//! (`migrations/0001_init_analytics.sql`). Set the connection string to run:
//!
//!   MIMIR_LAB_TEST_DB='mysql://root:PASS@localhost:13306/mimir' cargo test --test registry
//!
//! When unset the tests no-op (so offline / CI stay green). They use the
//! throwaway tenant `test_analytics` and clean up after themselves.

use mimir_lab::{NewDataset, PiiStatus, Registry};

const TENANT: &str = "test_analytics";

fn test_db() -> Option<String> {
    std::env::var("MIMIR_LAB_TEST_DB").ok()
}

async fn cleanup(reg: &Registry) {
    // remove any rows from prior runs (children first)
    let _ = sqlx::query(
        "DELETE dv FROM dataset_versions dv \
         JOIN datasets d ON d.id = dv.dataset_id WHERE d.tenant_id = ?",
    )
    .bind(TENANT)
    .execute(reg.pool())
    .await;
    let _ = sqlx::query("DELETE FROM datasets WHERE tenant_id = ?")
        .bind(TENANT)
        .execute(reg.pool())
        .await;
}

#[tokio::test]
async fn registry_dataset_lifecycle() {
    let Some(url) = test_db() else {
        eprintln!("skip registry_dataset_lifecycle: set MIMIR_LAB_TEST_DB to run");
        return;
    };
    let reg = Registry::connect(&url).await.expect("connect");
    cleanup(&reg).await;

    // register
    let id = reg
        .register_dataset(NewDataset {
            tenant_id: TENANT.into(),
            name: "sales_2026".into(),
            source_type: "upload".into(),
            schema_json: Some(r#"{"columns":[]}"#.into()),
            storage_uri: Some("minio://analytics/sales_2026".into()),
            row_count: 4,
            created_by: Some("test".into()),
        })
        .await
        .expect("register");

    // list → present, starts pending
    let list = reg.list_datasets(TENANT).await.expect("list");
    let row = list.iter().find(|d| d.id == id).expect("listed");
    assert_eq!(row.name, "sales_2026");
    assert_eq!(row.pii_status, "pending");
    assert_eq!(row.row_count, 4);

    // profile
    let got = reg.get_dataset(&id).await.expect("get").expect("some");
    assert_eq!(got.source_type, "upload");
    assert_eq!(got.storage_uri.as_deref(), Some("minio://analytics/sales_2026"));

    // version snapshot
    reg.record_version(&id, 1, Some("minio://analytics/sales_2026/v1.parquet"), Some("deadbeef"))
        .await
        .expect("version");

    // pii gate transitions: pending → flagged → clean
    reg.update_pii_status(&id, &PiiStatus::Flagged { categories: vec!["email".into(), "thai_national_id".into()] })
        .await
        .expect("flag");
    let flagged = reg.get_dataset(&id).await.unwrap().unwrap();
    assert_eq!(flagged.pii_status, "flagged");
    assert!(flagged.pii_categories.as_deref().unwrap().contains("email"));

    reg.update_pii_status(&id, &PiiStatus::Clean).await.expect("clean");
    let clean = reg.get_dataset(&id).await.unwrap().unwrap();
    assert_eq!(clean.pii_status, "clean");

    // delete
    reg.delete_dataset(&id).await.expect("delete");
    assert!(reg.get_dataset(&id).await.unwrap().is_none());

    cleanup(&reg).await;
}
