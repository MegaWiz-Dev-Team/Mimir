use axum::{extract::DefaultBodyLimit, middleware, routing::get, Extension, Json, Router};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;
use tracing_subscriber::{self, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use opentelemetry::global;
use opentelemetry_otlp::{WithExportConfig, WithTonicConfig};

use mimir_core_ai::middleware::request_id::request_id_middleware;
use mimir_core_ai::services::cron;
use mimir_core_ai::services::db;
use ro_ai_bridge::config::Config;
use ro_ai_bridge::routes::a2a::a2a_routes;
use ro_ai_bridge::routes::admin_knowledge::admin_knowledge_routes;
use ro_ai_bridge::routes::shared_knowledge::shared_knowledge_routes;
use ro_ai_bridge::routes::shared_kb_items::shared_kb_items_routes;
use ro_ai_bridge::routes::knowledge_search::knowledge_search_routes;
use ro_ai_bridge::routes::agents::agents_routes;
use ro_ai_bridge::routes::ask::ask_routes;
use ro_ai_bridge::routes::auth::auth_routes;
use ro_ai_bridge::routes::auto_pipeline::{batch_pipeline_routes, recover_orphaned_pipeline_runs};
use ro_ai_bridge::routes::backup::backup_routes;
use ro_ai_bridge::routes::budget::{budget_settings_routes, budget_usage_routes};
use ro_ai_bridge::routes::chat::chat_routes;
use ro_ai_bridge::routes::chunks::chunks_routes;
use ro_ai_bridge::routes::conversations::conversations_routes;
use ro_ai_bridge::routes::coverage::coverage_routes;
use ro_ai_bridge::routes::cron::{cron_routes, cron_status_routes};
use ro_ai_bridge::routes::db_connector::db_connector_routes;
use ro_ai_bridge::routes::docs::docs_routes;
use ro_ai_bridge::routes::eval::eval_routes;
use ro_ai_bridge::routes::training::training_routes;
use ro_ai_bridge::routes::icd10::icd10_routes;
use ro_ai_bridge::routes::rag_benchmark::rag_benchmark_routes;
use ro_ai_bridge::routes::evaluations_ext::evaluations_ext_routes;
use ro_ai_bridge::routes::feedback::feedback_routes;
use ro_ai_bridge::routes::assistant::assistant_routes;
use ro_ai_bridge::routes::graph::graph_routes;
use ro_ai_bridge::routes::iam::iam_routes;
use ro_ai_bridge::routes::ingest::ingest_routes;
use ro_ai_bridge::routes::llm_usage::llm_usage_routes;
use ro_ai_bridge::routes::mcp::mcp_routes;
use ro_ai_bridge::routes::models::models_routes;
use ro_ai_bridge::routes::ocr::ocr_routes;
use ro_ai_bridge::routes::pipeline::pipeline_routes;
use ro_ai_bridge::routes::prompts::prompts_routes;
use ro_ai_bridge::routes::qc::qc_routes;
use ro_ai_bridge::routes::tenant::tenant_routes;
use ro_ai_bridge::routes::tenant_query::tenant_query_routes;
use ro_ai_bridge::routes::vault::vault_routes;
use ro_ai_bridge::routes::vector::vector_routes;
// Sprint 32: RAG Ensemble Playground (Phase 2)
use ro_ai_bridge::routes::search::search_routes;
use ro_ai_bridge::routes::search_benchmark::search_benchmark_routes;
use ro_ai_bridge::routes::search_optimize::search_optimize_routes;
use ro_ai_bridge::routes::swarm::swarm_routes;
use ro_ai_bridge::routes::rag_eval::rag_eval_routes;
use ro_ai_bridge::routes::insurance::insurance_routes;

#[tokio::main]
async fn main() {
    // Initialize structured JSON logging with env-filter support
    // Usage: RUST_LOG=info (default), RUST_LOG=debug, RUST_LOG=ro_ai_bridge=debug,mimir_core_ai=info
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let fmt_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_target(true)
        .with_timer(tracing_subscriber::fmt::time::UtcTime::rfc_3339())
        .with_current_span(true);

    if let Ok(otlp_endpoint) = std::env::var("OTLP_ENDPOINT") {
        let mut exporter_builder = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(otlp_endpoint);

        if let Ok(auth_token) = std::env::var("VARDR_AUTH_TOKEN") {
            let mut metadata = tonic::metadata::MetadataMap::new();
            if let Ok(header_value) = format!("Bearer {}", auth_token).parse() {
                metadata.insert("authorization", header_value);
                exporter_builder = exporter_builder.with_metadata(metadata);
                tracing::info!("🔒 VARDR_AUTH_TOKEN successfully injected into OTLP exporter headers.");
            } else {
                tracing::warn!("⚠️ Failed to parse VARDR_AUTH_TOKEN as an authorization metadata value.");
            }
        }

        let tracer_provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
            .with_batch_exporter(
                exporter_builder.build().expect("Failed to build OTLP SpanExporter"),
            )
            .build();

        global::set_tracer_provider(tracer_provider.clone());
        let tracer = global::tracer("ro-ai-bridge");

        let telemetry_layer = tracing_opentelemetry::layer().with_tracer(tracer);

        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .with(telemetry_layer)
            .init();
        
        info!("🚀 OpenTelemetry tracing active. Exporting Spans to OTLP Endpoint.");
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .init();
    }

    // Inject Vault secrets into env vars (before Config reads them)
    mimir_core_ai::config::inject_vault_secrets().await;

    // Load configuration
    let config = Config::from_env();
    let config = Arc::new(config);

    // Initialize database
    let pool = db::init_db().await.expect("Failed to initialize database");
    info!(
        event = "db_connected",
        "✅ Database connected and migrations applied"
    );

    // Seed built-in roles (admin, editor, viewer) for all tenants (Issue #220)
    {
        let iam = mimir_core_ai::services::iam::IamService::new_with_env(pool.clone());
        if let Err(e) = iam.seed_builtin_roles_for_all_tenants().await {
            tracing::warn!(error = %e, "Failed to seed built-in roles on startup");
        }
    }

    // Synchronize remote models from Heimdall & Ollama (Issue #250)
    if let Err(e) = mimir_core_ai::services::model_sync::sync_models(&pool).await {
        tracing::warn!(error = %e, "Failed to synchronize remote LLM models on startup");
    }

    // Recover orphaned pipeline runs from previous pod lifecycle
    recover_orphaned_pipeline_runs(&pool).await;

    // Start cron worker for scheduled re-sync (Issue #150)
    let cron_tick_seconds: u64 = std::env::var("CRON_TICK_SECONDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60);
    let cron_state = cron::start_cron_worker(pool.clone(), cron_tick_seconds);

    // Sprint 52 — Yggdrasil JWT auth state (opt-in via YGGDRASIL_ISSUER + JWT_AUDIENCE
    // env vars). When unset, /api/v1/iam/* falls through to legacy HS256-only validation
    // using `config.jwt_secret`. Pattern: memory/asgard_jwt_auth_pattern.md
    // (Heimdall 0.6.0 = reference impl).
    if config.jwt_secret == "dev_secret_key" {
        tracing::warn!(
            event = "insecure_jwt_secret_default",
            "JWT_SECRET is the default 'dev_secret_key' — set the JWT_SECRET env var \
             before exposing this binary outside a dev box"
        );
    }
    let auth_state = Arc::new(
        mimir_core_ai::middleware::dual_mode_auth::AuthState::from_env(
            config.jwt_secret.clone(),
        ),
    );
    if auth_state.jwt_enabled() {
        info!(
            event = "yggdrasil_jwt_enabled",
            "Yggdrasil JWT validation active for /api/v1/iam/*"
        );
    } else {
        info!(
            event = "yggdrasil_jwt_disabled",
            "Yggdrasil JWT validation off — set YGGDRASIL_ISSUER to enable"
        );
    }

    // CORS layer for dashboard
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // build our application with routes
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/healthz", get(health_check))
        .merge(eval_routes())
        // Sprint 53: OCR LAYOUT eval storage (Syn v0.3.0+ region detection)
        // — asgard_platform tenant. Sibling to existing Sprint 51 ocr_eval_*
        // (text recognition; not nested here).
        .nest("/api/v1/eval/ocr/layout", ro_ai_bridge::routes::eval_ocr_layout::eval_ocr_layout_routes())
        // Sprint 39: Mimir Curator (annotation) + LoRA training tracking
        .merge(training_routes())
        // Sprint 48: ICD-10 / ICD-10-TM lookup (Hermodr-bound skill)
        .merge(icd10_routes())
        // Sprint 47 B-47g: clinician-curated rag_benchmark_items
        .merge(rag_benchmark_routes())
        .nest("/api/v1/app-settings", ro_ai_bridge::routes::app_settings::app_settings_routes())
        .nest("/api/v1", ro_ai_bridge::routes::auto_tune::auto_tune_routes())
        .nest("/api/v1", ro_ai_bridge::routes::insights::insights_routes())
        .nest("/api/v1/iam", iam_routes())
        .nest("/api/v1/auth", auth_routes())
        .nest("/api/v1/pipeline", pipeline_routes())
        .nest("/api/v1/qc", qc_routes())
        .nest("/api/v1", ro_ai_bridge::routes::stats::stats_routes())
        .nest("/api/v1/vector", vector_routes())
        .nest(
            "/api/v1/sources",
            ro_ai_bridge::routes::sources::sources_routes(),
        )
        .nest("/api/v1/chunks", chunks_routes())
        .nest("/api/v1/llm-usage", llm_usage_routes())
        .nest("/api/v1/agents", agents_routes())
        .nest("/api/v1/agents", chat_routes())
        .nest("/api/v1/conversations", conversations_routes())
        .nest("/api/v1/evaluations", evaluations_ext_routes())
        .nest("/api/v1/rag-eval", rag_eval_routes())
        .nest("/api/v1/settings", budget_settings_routes())
        .merge(budget_usage_routes())
        // Sprint 14: Cron schedule, feedback & OCR routes
        .nest("/api/v1", cron_routes())
        .nest("/api/v1", cron_status_routes())
        .nest("/api/v1/feedback", feedback_routes())
        .nest("/api/v1/assistant", assistant_routes())
        .nest("/api/v1", ocr_routes())
        // Sprint 50b — Skuggi PII test corpus admin (B-50b)
        .nest("/api/v1", ro_ai_bridge::routes::admin_skuggi::admin_skuggi_routes())
        .nest("/api/v1", batch_pipeline_routes())
        .nest("/api/v1/db-connector", db_connector_routes())
        .nest("/api/v1", models_routes())
        .nest("/api/v1/vault", vault_routes())
        .nest("/api/v1/mcp", mcp_routes())
        .nest("/api/v1/backup", backup_routes())
        .nest("/api/docs", docs_routes())
        // Sprint 17: Knowledge Graph routes
        .nest("/api/v1/graph", graph_routes())
        .nest("/api/v1/admin/knowledge", admin_knowledge_routes())
        .nest("/api/v1/knowledge/shared", shared_knowledge_routes())
        .nest("/api/v1/knowledge/shared", shared_kb_items_routes())
        .nest("/api/v1/knowledge/search", knowledge_search_routes())
        // Sprint 18: Coverage Analytics routes
        .nest("/api/v1/coverage", coverage_routes())
        .nest("/api/v1/prompts", prompts_routes())
        // Sprint 29: Simple RAG Q&A
        .merge(ask_routes())
        // Sprint 30: Tenant Management + PageIndex
        .nest("/api/v1/tenants", tenant_routes())
        .nest("/api/v1/tenants/{tenant_id}/ingest", ingest_routes())
        .nest("/api/v1/tenants/{tenant_id}/query", tenant_query_routes())
        .nest("/api/v1", ro_ai_bridge::routes::features::features_routes())
        // Sprint 32: RAG Ensemble Playground (Phase 2)
        .merge(search_routes())
        .merge(search_optimize_routes())
        .merge(search_benchmark_routes())
        // Sprint 18: Swarm Multi Agent
        .nest("/api/v1/tenants/{tenant_id}", swarm_routes())
        // Sprint 52: Insurance Underwriting Sidecar Backend Mocks
        .nest("/api/v1/insurance", insurance_routes())
        // Phase 2: Agent-to-Agent Cross-Tenant Routing
        .nest("/api/v1/a2a", a2a_routes())
        .layer(middleware::from_fn(request_id_middleware))
        .with_state(pool)
        .layer(Extension(config.clone()))
        .layer(Extension(auth_state))
        .layer(Extension(cron_state))
        .layer(DefaultBodyLimit::max(500 * 1024 * 1024))
        .layer(cors);

    // run it
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    info!(event = "server_starting", address = %addr, "🚀 listening on {}", addr);

    let listener = TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health_check() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "ro-ai-bridge",
        "version": env!("CARGO_PKG_VERSION")
    }))
}
