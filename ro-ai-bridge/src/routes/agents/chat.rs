//! Agent chat and conversation listing.

use crate::routes::tenant::extract_tenant_id;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Extension, Json,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{error, info};
use uuid::Uuid;

use crate::config::Config;
use crate::routes::llm_usage::insert_llm_usage_log;
use crate::routes::sources::resolve_llm_credentials;
use mimir_core_ai::services::db::DbPool;

use super::crud::{
    AgentChatRequest, AgentChatResponse, AgentConfig, ConversationListQuery, ConversationSession,
    AGENT_SELECT_COLS,
};

/// POST /api/v1/agents/:id/chat — Chat with agent using its config
pub(crate) async fn agent_chat(
    headers: HeaderMap,
    Extension(config): Extension<Arc<Config>>,
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
    Json(payload): Json<AgentChatRequest>,
) -> Result<Json<AgentChatResponse>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);

    // 1. Load agent config
    let agent = sqlx::query_as::<_, AgentConfig>(&format!(
        "SELECT {} FROM agent_configs WHERE id = ? AND tenant_id = ?",
        AGENT_SELECT_COLS
    ))
    .bind(id)
    .bind(tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?
    .ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Agent not found"})),
        )
    })?;

    let session_id = payload
        .session_id
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    // 2. Log user message
    let _ = sqlx::query(
        r#"INSERT INTO agent_conversations
            (tenant_id, agent_config_id, session_id, role, content, model_id)
        VALUES (?, ?, ?, 'user', ?, ?)"#,
    )
    .bind(tenant_id)
    .bind(id)
    .bind(&session_id)
    .bind(&payload.message)
    .bind(&agent.model_id)
    .execute(&pool)
    .await;

    // 3. Resolve LLM credentials
    let model_config = mimir_core_ai::services::db::get_model_by_id(&pool, &agent.model_id)
        .await
        .map_err(|e| {
            error!("Failed to look up model {}: {}", agent.model_id, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Model lookup failed: {}", e)})),
            )
        })?;

    let (api_key, api_base) = resolve_llm_credentials(&config, &model_config, &agent.model_id)?;

    // 4. Ensemble RAG augmentation (Vector + Tree + Graph + PrimeKG)
    let mut system_prompt = agent.system_prompt.clone();
    let use_rag = agent.use_rag.unwrap_or(true);
    let use_kg = agent.use_knowledge_graph.unwrap_or(false);
    let mut reasoning_msg = None;
    let mut chat_trace: Option<serde_json::Value> = None;

    // Parse tools list — drives optional global knowledge collections
    let agent_tools: Vec<String> = agent.tools
        .as_ref()
        .and_then(|v| serde_json::from_value::<Vec<String>>(v.clone()).ok())
        .unwrap_or_default();
    let use_primekg = agent_tools.iter().any(|t| t == "primekg_search");
    let use_clinical = agent_tools.iter().any(|t| t == "clinical_kb_search");

    if use_rag || use_kg || use_primekg || use_clinical {
        use crate::retrieval::EnsembleWeights;
        use crate::routes::search::run_parallel_search_filtered;
        use crate::routes::search::SearchFilters;

        // Parse rag_params if available, otherwise use defaults
        let rag_params = agent.rag_params.as_ref();
        let weights = rag_params
            .and_then(|p| p.get("weights"))
            .and_then(|w| serde_json::from_value::<EnsembleWeights>(w.clone()).ok())
            .unwrap_or_else(|| {
                // Respect individual toggles: if use_rag is off, zero vector/tree
                let mut w = EnsembleWeights::default();
                if !use_rag {
                    w.vector = 0.0;
                    w.tree = 0.0;
                }
                if !use_kg {
                    w.graph = 0.0;
                }
                let use_tree = agent.use_pageindex.unwrap_or(false);
                if !use_tree {
                    w.tree = 0.0;
                }
                w.normalize();
                w
            });

        let top_k = agent.top_k.unwrap_or(5) as usize;

        // Add global collections only when agent has the corresponding tool enabled
        let mut extra_collections: Vec<&str> = Vec::new();
        if use_primekg { extra_collections.push("primekg-entities"); }
        if use_clinical { extra_collections.push("clinical-wisdom"); }
        let extra: Option<&[&str]> = if extra_collections.is_empty() {
            None
        } else {
            Some(&extra_collections)
        };

        // ─── Wave 3: capture retrieval params + timing for experiment tracking ───
        let retrieval_alpha = 0.7;
        let retrieval_threshold = 0.0;
        let retrieval_hop_limit = 2;
        let retrieval_t0 = std::time::Instant::now();

        // ─── Sprint 37 B-23: optional query expansion ─────────────────────────
        //
        // Toggle via QUERY_EXPANSION_N=N env var (1..=5; default 0=off). When N>1,
        // the user's question is rewritten by Gemini Flash into N paraphrases
        // (medical synonyms, alternate phrasings). All paraphrases are passed to
        // the search call as additional queries — recall improves at the cost of
        // 1 extra LLM call per request.
        //
        // Implementation note: we only do the LLM rewrite when N≥2. The expanded
        // queries are then concatenated into the search query field (separated
        // by " || ") which both BGE-M3 dense and BM25 sparse handle as additive
        // signal (the embed model sees them as one passage; BM25 OR-merges terms).
        let query_expansion_n: u32 = std::env::var("QUERY_EXPANSION_N")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let expanded_query: String = if query_expansion_n >= 2 {
            // Use the cheap default Gemini judge model — already configured + reachable.
            let gemini_model = std::env::var("QUERY_EXPANSION_MODEL")
                .unwrap_or_else(|_| "gemini-2.5-flash".to_string());
            let prompt = format!(
                "Rewrite this medical question in exactly {} different but semantically equivalent ways. \
                 Use medical synonyms, alternate phrasings, and clinical terminology variations. \
                 Output ONLY the rewrites separated by newlines, no numbering or commentary.\n\n\
                 Question: {}",
                query_expansion_n, payload.message
            );
            let cfg = mimir_core_ai::services::gemini_helper::GeminiCallConfig {
                temperature: 0.5, max_output_tokens: 256, force_json: false, timeout_secs: 15,
            };
            match mimir_core_ai::services::gemini_helper::call_text(&gemini_model, &prompt, &cfg).await {
                Ok(res) => {
                    let paraphrases: Vec<&str> = res.text.lines()
                        .map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
                    if paraphrases.is_empty() {
                        payload.message.clone()
                    } else {
                        // Original + paraphrases joined with " || " — BGE-M3 treats as one
                        // passage (dense), BM25 OR-merges tokens (sparse).
                        let mut all = vec![payload.message.as_str()];
                        all.extend(&paraphrases);
                        all.join(" || ")
                    }
                }
                Err(e) => {
                    tracing::warn!(err=%e, "Query expansion LLM call failed; using original");
                    payload.message.clone()
                }
            }
        } else {
            payload.message.clone()
        };

        // ─── Sprint 36 B-16 + per-model gating: cross-encoder rerank ──────────
        //
        // Resolution order (highest precedence first):
        //   1. RERANKER_ENABLED=1  → force ON  (for A/B testing, ad-hoc tournaments)
        //   2. RERANKER_ENABLED=0  → force OFF
        //   3. ai_models.metadata.rerank_recommended for the agent's model
        //   4. Default OFF
        //
        // Why per-model? Sprint 36 Phase 2 found rerank helps Flash-class cloud
        // models (+4pp on flash-lite) but hurts larger reasoning models that use
        // peripheral context for synthesis (-9pp on gemma-4-26b, -7pp on 2.5-flash).
        // The bge-reranker-v2-m3 is general-domain and trims context too aggressively
        // for the larger models — until we have a medical-domain reranker, gate
        // per-model based on empirical evidence stored in ai_models.metadata.
        let rerank_recommended = if !agent.model_id.is_empty() {
            sqlx::query_scalar::<_, Option<bool>>(
                "SELECT JSON_EXTRACT(metadata, '$.rerank_recommended') = TRUE
                   FROM ai_models WHERE model_id = ?"
            )
            .bind(&agent.model_id)
            .fetch_optional(&pool)
            .await
            .ok()
            .flatten()
            .flatten()
            .unwrap_or(false)
        } else {
            false
        };
        let rerank_enabled = match std::env::var("RERANKER_ENABLED").as_deref() {
            Ok("1") => true,
            Ok("0") => false,
            _ => rerank_recommended,
        };
        let rerank_cfg = if rerank_enabled {
            Some(crate::routes::rag_eval::RerankConfig {
                enabled: true,
                strategy: "cross-encoder".to_string(),
                model: std::env::var("RERANKER_MODEL").ok()
                    .or_else(|| Some("BAAI/bge-reranker-v2-m3".to_string())),
                final_top_k: top_k,
            })
        } else {
            None
        };

        let rag_results = run_parallel_search_filtered(
            &pool,
            &expanded_query, // Sprint 37 B-23: original or paraphrase-expanded query
            tenant_id,
            &weights,
            top_k,
            &SearchFilters::default(),
            rerank_cfg.as_ref(),
            retrieval_alpha,
            retrieval_threshold,
            retrieval_hop_limit,
            extra,
        )
        .await;
        let retrieval_latency_ms = retrieval_t0.elapsed().as_millis() as i64;

        if !rag_results.is_empty() {
            // Group results by source type for structured injection
            let mut vector_ctx = Vec::new();
            let mut tree_ctx = Vec::new();
            let mut graph_ctx = Vec::new();
            let mut primekg_ctx = Vec::new();
            let mut clinical_ctx = Vec::new();

            for r in &rag_results {
                let entry = format!("• [{}] {}", r.title, r.content);
                match r.source_type.as_str() {
                    "vector" => vector_ctx.push(entry),
                    "tree" => tree_ctx.push(entry),
                    "graph" => graph_ctx.push(entry),
                    "primekg" => primekg_ctx.push(entry),
                    "clinical" => clinical_ctx.push(entry),
                    _ => vector_ctx.push(entry),
                }
            }

            let mut context_sections = Vec::new();

            if !vector_ctx.is_empty() {
                context_sections.push(format!(
                    "[Retrieved Context — Vector Search]\n{}",
                    vector_ctx.join("\n")
                ));
            }
            if !tree_ctx.is_empty() {
                context_sections.push(format!(
                    "[Retrieved Context — Document Structure]\n{}",
                    tree_ctx.join("\n")
                ));
            }
            if !graph_ctx.is_empty() {
                context_sections.push(format!(
                    "[Knowledge Graph Context]\nThe following entities and relationships are relevant:\n{}",
                    graph_ctx.join("\n")
                ));
            }
            if !primekg_ctx.is_empty() {
                context_sections.push(format!(
                    "[PrimeKG Medical Knowledge]\nThe following medical entities from the global knowledge base are relevant:\n{}",
                    primekg_ctx.join("\n")
                ));
            }
            if !clinical_ctx.is_empty() {
                context_sections.push(format!(
                    "[Clinical Knowledge Base]\nThe following clinical guidelines and reference material are relevant:\n{}",
                    clinical_ctx.join("\n")
                ));
            }

            let rag_section = format!(
                "\n\n{}\n\nUse the above context to answer the user's question. If the context does not contain relevant information, say so honestly.",
                context_sections.join("\n\n")
            );
            system_prompt.push_str(&rag_section);

            info!(
                event = "rag_augmented",
                agent_id = id,
                total_chunks = rag_results.len(),
                vector = vector_ctx.len(),
                tree = tree_ctx.len(),
                graph = graph_ctx.len(),
                primekg = primekg_ctx.len(),
                clinical = clinical_ctx.len(),
                "Ensemble RAG context injected"
            );
            reasoning_msg = Some(format!(
                "RAG Engine: Augmented prompt with {} context chunks (Vector: {}, Tree: {}, Graph: {}, PrimeKG: {}, Clinical: {}).",
                rag_results.len(), vector_ctx.len(), tree_ctx.len(), graph_ctx.len(), primekg_ctx.len(), clinical_ctx.len()
            ));

            // ─── Wave 3: structured trace for experiment tracking ───
            // Sprint 47 B-47c: include stable `chunk_id` so eval runner can
            // persist retrieved_chunk_ids and downstream RAGAS / retrieval
            // metrics can correlate against rag_benchmark_items gold sets.
            let chunks_json: Vec<serde_json::Value> = rag_results.iter().map(|r| {
                let content_preview: String = r.content.chars().take(200).collect();
                // Resolve chunk_id from metadata (Qdrant payload usually has
                // 'id' or 'chunk_id'); fall back to a deterministic
                // {source}:{title} composite when absent. This keeps IDs
                // stable across runs without forcing a schema change in the
                // upstream knowledge bases.
                let chunk_id = r.metadata.get("chunk_id")
                    .or_else(|| r.metadata.get("id"))
                    .and_then(|v| v.as_str().map(|s| s.to_string())
                        .or_else(|| v.as_i64().map(|n| n.to_string()))
                        .or_else(|| v.as_u64().map(|n| n.to_string())))
                    .unwrap_or_else(|| format!("{}:{}", r.source_type, r.title));
                serde_json::json!({
                    "chunk_id": chunk_id,
                    "source": r.source_type,
                    "title": r.title,
                    "score": r.score,
                    "content_preview": content_preview,
                })
            }).collect();
            chat_trace = Some(serde_json::json!({
                "retrieval_params": {
                    "alpha": retrieval_alpha,
                    "threshold": retrieval_threshold,
                    "hop_limit": retrieval_hop_limit,
                    "top_k": top_k,
                    "weights": {
                        "vector": weights.vector,
                        "tree": weights.tree,
                        "graph": weights.graph,
                    },
                    "extra_collections": extra_collections,
                    // Sprint 36 B-16: rerank metadata for trace replay
                    "rerank": rerank_cfg.as_ref().map(|c| serde_json::json!({
                        "enabled": c.enabled,
                        "strategy": c.strategy,
                        "model": c.model,
                        "final_top_k": c.final_top_k,
                    })),
                },
                "retrieval_counts": {
                    "vector": vector_ctx.len(),
                    "tree": tree_ctx.len(),
                    "graph": graph_ctx.len(),
                    "primekg": primekg_ctx.len(),
                    "clinical": clinical_ctx.len(),
                    "total": rag_results.len(),
                },
                "retrieval_chunks": chunks_json,
                "step_timings_ms": {
                    "retrieval": retrieval_latency_ms,
                },
                "tools_enabled": agent_tools,
            }));
        } else {
            reasoning_msg = Some("RAG Engine: No relevant context found for the query.".to_string());
            chat_trace = Some(serde_json::json!({
                "retrieval_params": {
                    "alpha": retrieval_alpha, "threshold": retrieval_threshold,
                    "hop_limit": retrieval_hop_limit, "top_k": top_k,
                    "weights": {"vector": weights.vector, "tree": weights.tree, "graph": weights.graph},
                    "extra_collections": extra_collections,
                },
                "retrieval_counts": {"total": 0},
                "retrieval_chunks": [],
                "step_timings_ms": {"retrieval": retrieval_latency_ms},
                "tools_enabled": agent_tools,
            }));
        }
    } else {
        reasoning_msg = Some("Mimir Engine: RAG & Knowledge Graph are disabled.".to_string());
    }

    // 5. Build prompt with system prompt + user message
    // Sprint 37 B-22 fix: per-call temperature override via X-Sampling-Temperature
    // header. Used by the eval runner for self-consistency sampling — bypass the
    // production-tuned low temp (0.3) to get diverse samples without modifying
    // agent_configs.
    let temperature = headers.get("x-sampling-temperature")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or_else(|| agent.temperature.unwrap_or(0.7));
    let max_tokens = agent.max_tokens.unwrap_or(2048);

    let start = std::time::Instant::now();
    let client = reqwest::Client::new();
    let url = format!("{}chat/completions", api_base);

    // Build messages array with conversation history
    let mut messages = vec![json!({"role": "system", "content": system_prompt})];

    // Load recent history for context (last 10 messages)
    let history: Vec<(String, String)> = sqlx::query_as(
        r#"SELECT role, content FROM agent_conversations
        WHERE session_id = ? AND agent_config_id = ?
        ORDER BY created_at DESC LIMIT 10"#,
    )
    .bind(&session_id)
    .bind(id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    // Add history in chronological order (excluding the just-inserted user message)
    for (role, content) in history.iter().rev().skip(1) {
        messages.push(json!({"role": role, "content": content}));
    }

    // Add current user message
    messages.push(json!({"role": "user", "content": payload.message}));

    let body = json!({
        "model": agent.model_id,
        "messages": messages,
        "max_tokens": max_tokens,
        "temperature": temperature
    });

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            error!("Agent chat HTTP error: {}", e);
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": format!("LLM call failed: {}", e)})),
            )
        })?;

    let latency_ms = start.elapsed().as_millis() as i32;

    if !response.status().is_success() {
        let error_body = response.text().await.unwrap_or_default();
        error!("Agent chat LLM error: {}", error_body);

        // Log error usage
        let provider_str = model_config
            .as_ref()
            .map(|m| m.provider.as_str())
            .unwrap_or("unknown");
        let _ = insert_llm_usage_log(
            &pool,
            tenant_id,
            &agent.model_id,
            provider_str,
            Some(&url),
            Some("agent_chat"),
            0,
            0,
            0,
            latency_ms,
            "error",
            Some(&error_body),
        )
        .await;

        return Err((
            StatusCode::BAD_GATEWAY,
            Json(json!({"error": format!("LLM error: {}", error_body)})),
        ));
    }

    let resp_json: Value = response.json().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Parse failed: {}", e)})),
        )
    })?;

    // Sprint 38f follow-up: reasoning models (Qwen3 thinking, DeepSeek R1, GPT-OSS,
    // Qwen-3-32B-Medical-Reasoning) put their answer in `message.reasoning` instead
    // of `message.content` when thinking mode is on. Concatenate both so the agent's
    // answer reaches the judge — content first (final answer), then reasoning if any.
    let content_raw = resp_json["choices"][0]["message"]["content"]
        .as_str().unwrap_or("").to_string();
    let reasoning_raw = resp_json["choices"][0]["message"]["reasoning"]
        .as_str().unwrap_or("").to_string();
    let content = match (content_raw.trim().is_empty(), reasoning_raw.trim().is_empty()) {
        (false, false) => format!("{}\n\n{}", content_raw, reasoning_raw),
        (true, false)  => reasoning_raw.clone(),
        _              => content_raw.clone(),
    };

    let input_tokens = resp_json["usage"]["prompt_tokens"].as_i64().unwrap_or(0) as i32;
    let output_tokens = resp_json["usage"]["completion_tokens"]
        .as_i64()
        .unwrap_or(0) as i32;
    let total_tokens = resp_json["usage"]["total_tokens"].as_i64().unwrap_or(0) as i32;

    // 5. Log usage
    let provider_str = model_config
        .as_ref()
        .map(|m| m.provider.as_str())
        .unwrap_or(&agent.provider);
    let _ = insert_llm_usage_log(
        &pool,
        tenant_id,
        &agent.model_id,
        provider_str,
        Some(&url),
        Some("agent_chat"),
        input_tokens,
        output_tokens,
        total_tokens,
        latency_ms,
        "success",
        None,
    )
    .await;

    // 6. Log assistant message to conversation
    let _ = sqlx::query(
        r#"INSERT INTO agent_conversations
            (tenant_id, agent_config_id, session_id, role, content, model_id, latency_ms, input_tokens, output_tokens)
        VALUES (?, ?, ?, 'assistant', ?, ?, ?, ?, ?)"#
    )
    .bind(tenant_id)
    .bind(id)
    .bind(&session_id)
    .bind(&content)
    .bind(&agent.model_id)
    .bind(latency_ms)
    .bind(input_tokens)
    .bind(output_tokens)
    .execute(&pool)
    .await;

    info!(
        "Agent chat id={} session={} latency={}ms tokens={}",
        id, session_id, latency_ms, total_tokens
    );

    // ─── Wave 3: enrich trace with generation timing ───
    if let Some(ref mut t) = chat_trace {
        // Snapshot retrieval ms before mutating
        let retrieval_ms = t.get("step_timings_ms")
            .and_then(|v| v.get("retrieval"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        if let Some(timings) = t.get_mut("step_timings_ms").and_then(|v| v.as_object_mut()) {
            timings.insert("generation".to_string(), serde_json::json!(latency_ms));
            timings.insert("total".to_string(), serde_json::json!(latency_ms + retrieval_ms));
        }
        if let Some(obj) = t.as_object_mut() {
            obj.insert("model".to_string(), serde_json::json!(&agent.model_id));
            obj.insert("provider".to_string(), serde_json::json!(&agent.provider));
            obj.insert("temperature".to_string(), serde_json::json!(temperature));
            obj.insert("max_tokens".to_string(), serde_json::json!(max_tokens));
            obj.insert("input_tokens".to_string(), serde_json::json!(input_tokens));
            obj.insert("output_tokens".to_string(), serde_json::json!(output_tokens));
        }
    }

    Ok(Json(AgentChatResponse {
        content,
        session_id,
        model_id: agent.model_id,
        provider: agent.provider,
        latency_ms,
        input_tokens,
        output_tokens,
        confidence_score: None,
        reasoning: reasoning_msg,
        trace: chat_trace,
    }))
}

/// GET /api/v1/agents/:id/conversations — List conversation sessions
pub(crate) async fn list_agent_conversations(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
    Query(params): Query<ConversationListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(20).min(100);
    let offset = (page - 1) * per_page;

    let sessions: Vec<ConversationSession> = sqlx::query_as(
        r#"SELECT
            session_id,
            agent_config_id,
            COUNT(*) as message_count,
            MIN(created_at) as first_message_at,
            MAX(created_at) as last_message_at
        FROM agent_conversations
        WHERE tenant_id = ? AND agent_config_id = ?
        GROUP BY session_id, agent_config_id
        ORDER BY last_message_at DESC
        LIMIT ? OFFSET ?"#,
    )
    .bind(tenant_id)
    .bind(id)
    .bind(per_page)
    .bind(offset)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;

    Ok(Json(json!({
        "sessions": sessions,
        "page": page,
        "per_page": per_page
    })))
}
