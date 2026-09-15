//! Data files and code defaults must not name a retired Gemini 2.5 model (Vertex/Gemini API
//! shutdown 2027-01-28 for 2.5 Flash-Lite, 2027-03-31 for 2.5 Flash and Pro).

use std::path::Path;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn persona_configs_name_a_live_gemini_model() {
    let dir = root().join("config/personas");
    for entry in std::fs::read_dir(&dir).expect("personas dir") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("gemini-2.5"), "{} still pins a retired Gemini 2.5 model", path.display());
    }
}

#[test]
fn a_migration_moves_stored_gemini_25_slots_to_gemini_3() {
    let dir = root().join("mimir-core-ai/migrations");
    let found = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| std::fs::read_to_string(e.path()).unwrap_or_default())
        .any(|sql| sql.contains("gemini-3.5-flash-lite") && sql.contains("$.judge.model") && sql.contains("is_active"));
    assert!(found, "no migration rewrites tenant llm_config slots / persona model_id / ai_models rows away from gemini-2.5");
}

#[test]
fn code_defaults_do_not_name_gemini_25() {
    for rel in [
        "src/config.rs",
        "src/routes/agents/chat.rs",
        "src/bin/run_eval.rs",
        "src/bin/generate_qa.rs",
        "mimir-core-ai/src/services/gemini_helper.rs",
    ] {
        let text = std::fs::read_to_string(root().join(rel)).unwrap();
        let hits: Vec<_> = text
            .lines()
            .enumerate()
            .filter(|(_, l)| l.contains("\"gemini-2.5"))
            .map(|(i, l)| format!("{rel}:{}: {}", i + 1, l.trim()))
            .collect();
        assert!(hits.is_empty(), "retired default(s):\n{}", hits.join("\n"));
    }
}
