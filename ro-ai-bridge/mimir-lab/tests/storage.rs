//! MinIO/S3 storage integration test — env-gated.
//!
//! Needs a reachable MinIO with the bucket already created. Run:
//!
//!   S3_ENDPOINT=http://localhost:19000 S3_BUCKET=asgard-analytics \
//!   S3_ACCESS_KEY=... S3_SECRET_KEY=... MIMIR_LAB_TEST_MINIO=1 \
//!   cargo test --test storage -- --nocapture
//!
//! When `MIMIR_LAB_TEST_MINIO` is unset the test no-ops (offline/CI stay green).

use mimir_lab::Storage;

#[tokio::test]
async fn storage_put_get_delete_round_trip() {
    if std::env::var("MIMIR_LAB_TEST_MINIO").is_err() {
        eprintln!("skip storage round-trip: set MIMIR_LAB_TEST_MINIO=1 (+ S3_* env) to run");
        return;
    }
    let st = Storage::from_env().expect("storage from env");
    let key = "test/_mimir_lab_probe.txt";
    let body = b"hello-mimir-lab";

    let uri = st.put(key, body).await.expect("put");
    assert!(uri.starts_with("s3://"));

    let got = st.get(key).await.expect("get");
    assert_eq!(got, body);

    st.delete(key).await.expect("delete");
}
