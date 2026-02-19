use anyhow::Result;
use dotenvy::dotenv;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use tracing::{info, warn, error};
use tracing_subscriber;

// ─── YAML Data Structures ──────────────────────────────────

#[derive(Debug, Deserialize)]
struct YamlItemRoot {
    #[serde(rename = "Body")]
    body: Option<Vec<YamlItem>>,
}

#[derive(Debug, Deserialize)]
struct YamlItem {
    #[serde(rename = "Id")]
    id: u32,
    #[serde(rename = "AegisName")]
    aegis_name: Option<String>,
    #[serde(rename = "Name")]
    name: Option<String>,
    #[serde(rename = "Type")]
    item_type: Option<String>,
    #[serde(rename = "SubType")]
    sub_type: Option<String>,
    #[serde(rename = "Buy")]
    buy: Option<u32>,
    #[serde(rename = "Weight")]
    weight: Option<u32>,
    #[serde(rename = "Attack")]
    attack: Option<u32>,
    #[serde(rename = "MagicAttack")]
    magic_attack: Option<u32>,
    #[serde(rename = "Defense")]
    defense: Option<u32>,
    #[serde(rename = "Range")]
    range: Option<u32>,
    #[serde(rename = "Slots")]
    slots: Option<u32>,
    #[serde(rename = "WeaponLevel")]
    weapon_level: Option<u32>,
    #[serde(rename = "ArmorLevel")]
    armor_level: Option<u32>,
    #[serde(rename = "EquipLevelMin")]
    equip_level_min: Option<u32>,
    #[serde(rename = "Refineable")]
    refineable: Option<bool>,
    #[serde(rename = "Locations")]
    locations: Option<serde_json::Value>,
    #[serde(rename = "Jobs")]
    jobs: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct YamlMobRoot {
    #[serde(rename = "Body")]
    body: Option<Vec<YamlMob>>,
}

#[derive(Debug, Deserialize)]
struct YamlMob {
    #[serde(rename = "Id")]
    id: u32,
    #[serde(rename = "AegisName")]
    aegis_name: Option<String>,
    #[serde(rename = "Name")]
    name: Option<String>,
    #[serde(rename = "Level")]
    level: Option<u32>,
    #[serde(rename = "Hp")]
    hp: Option<u64>,
    #[serde(rename = "Sp")]
    sp: Option<u32>,
    #[serde(rename = "BaseExp")]
    base_exp: Option<u64>,
    #[serde(rename = "JobExp")]
    job_exp: Option<u64>,
    #[serde(rename = "MvpExp")]
    mvp_exp: Option<u64>,
    #[serde(rename = "Attack")]
    attack: Option<u32>,
    #[serde(rename = "Attack2")]
    attack2: Option<u32>,
    #[serde(rename = "Defense")]
    defense: Option<u32>,
    #[serde(rename = "MagicDefense")]
    magic_defense: Option<u32>,
    #[serde(rename = "Str")]
    str_stat: Option<u32>,
    #[serde(rename = "Agi")]
    agi: Option<u32>,
    #[serde(rename = "Vit")]
    vit: Option<u32>,
    #[serde(rename = "Int")]
    int_stat: Option<u32>,
    #[serde(rename = "Dex")]
    dex: Option<u32>,
    #[serde(rename = "Luk")]
    luk: Option<u32>,
    #[serde(rename = "Size")]
    size: Option<String>,
    #[serde(rename = "Race")]
    race: Option<String>,
    #[serde(rename = "Element")]
    element: Option<String>,
    #[serde(rename = "ElementLevel")]
    element_level: Option<u32>,
    #[serde(rename = "WalkSpeed")]
    walk_speed: Option<u32>,
    #[serde(rename = "AttackRange")]
    attack_range: Option<u32>,
    #[serde(rename = "SkillRange")]
    skill_range: Option<u32>,
    #[serde(rename = "ChaseRange")]
    chase_range: Option<u32>,
    #[serde(rename = "Ai")]
    ai: Option<String>,
    #[serde(rename = "Class")]
    mob_class: Option<String>,
}

// ─── Embedding API ─────────────────────────────────────────

#[derive(Debug, Serialize)]
struct EmbedRequest {
    model: String,
    input: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct EmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

async fn get_embeddings(client: &Client, ollama_url: &str, model: &str, texts: &[String]) -> Result<Vec<Vec<f32>>> {
    let req = EmbedRequest {
        model: model.to_string(),
        input: texts.to_vec(),
    };

    let resp = client
        .post(format!("{}/api/embed", ollama_url))
        .json(&req)
        .send()
        .await?;

    if !resp.status().is_success() {
        let err = resp.text().await?;
        return Err(anyhow::anyhow!("Ollama embed error: {}", err));
    }

    let embed_resp: EmbedResponse = resp.json().await?;
    Ok(embed_resp.embeddings)
}

// ─── Text Builders ─────────────────────────────────────────

fn item_to_text(item: &YamlItem) -> String {
    let name = item.name.as_deref().unwrap_or("Unknown");
    let item_type = item.item_type.as_deref().unwrap_or("Etc");
    let sub_type = item.sub_type.as_ref().map(|s| format!(" ({})", s)).unwrap_or_default();

    let mut parts = vec![format!("{} — Type: {}{}", name, item_type, sub_type)];

    if let Some(atk) = item.attack { if atk > 0 { parts.push(format!("ATK: {}", atk)); } }
    if let Some(matk) = item.magic_attack { if matk > 0 { parts.push(format!("MATK: {}", matk)); } }
    if let Some(def) = item.defense { if def > 0 { parts.push(format!("DEF: {}", def)); } }
    if let Some(w) = item.weight { parts.push(format!("Weight: {}", w as f32 / 10.0)); }
    if let Some(s) = item.slots { if s > 0 { parts.push(format!("Slots: {}", s)); } }
    if let Some(b) = item.buy { if b > 0 { parts.push(format!("Price: {} zeny", b)); } }
    if let Some(wl) = item.weapon_level { parts.push(format!("Weapon Lv: {}", wl)); }
    if let Some(al) = item.armor_level { parts.push(format!("Armor Lv: {}", al)); }
    if let Some(el) = item.equip_level_min { if el > 0 { parts.push(format!("Req Lv: {}", el)); } }
    if let Some(r) = item.range { if r > 0 { parts.push(format!("Range: {}", r)); } }
    if item.refineable == Some(true) { parts.push("Refineable".to_string()); }

    // Locations
    if let Some(locs) = &item.locations {
        if let Some(obj) = locs.as_object() {
            let loc_names: Vec<&str> = obj.keys().map(|k| k.as_str()).collect();
            if !loc_names.is_empty() { parts.push(format!("Equip: {}", loc_names.join(", "))); }
        }
    }

    // Jobs
    if let Some(jobs) = &item.jobs {
        if let Some(obj) = jobs.as_object() {
            let job_names: Vec<&str> = obj.keys().map(|k| k.as_str()).collect();
            if !job_names.is_empty() && job_names.len() < 15 {
                parts.push(format!("Jobs: {}", job_names.join(", ")));
            }
        }
    }

    parts.join(", ")
}

fn mob_to_text(mob: &YamlMob) -> String {
    let name = mob.name.as_deref().unwrap_or("Unknown");
    let level = mob.level.unwrap_or(1);

    let mut parts = vec![format!("{} — Level {}", name, level)];

    if let Some(hp) = mob.hp { parts.push(format!("HP: {}", hp)); }
    if let Some(atk) = mob.attack { parts.push(format!("ATK: {}", atk)); }
    if let Some(def) = mob.defense { if def > 0 { parts.push(format!("DEF: {}", def)); } }
    if let Some(mdef) = mob.magic_defense { if mdef > 0 { parts.push(format!("MDEF: {}", mdef)); } }
    if let Some(bexp) = mob.base_exp { if bexp > 0 { parts.push(format!("Base EXP: {}", bexp)); } }
    if let Some(jexp) = mob.job_exp { if jexp > 0 { parts.push(format!("Job EXP: {}", jexp)); } }
    if let Some(mexp) = mob.mvp_exp { if mexp > 0 { parts.push(format!("MVP EXP: {}", mexp)); } }
    if let Some(s) = &mob.size { parts.push(format!("Size: {}", s)); }
    if let Some(r) = &mob.race { parts.push(format!("Race: {}", r)); }
    if let Some(e) = &mob.element {
        let el = mob.element_level.unwrap_or(1);
        parts.push(format!("Element: {} Lv{}", e, el));
    }
    if let Some(ar) = mob.attack_range { if ar > 0 { parts.push(format!("ATK Range: {}", ar)); } }

    // Stats summary
    let stats: Vec<String> = [
        ("STR", mob.str_stat), ("AGI", mob.agi), ("VIT", mob.vit),
        ("INT", mob.int_stat), ("DEX", mob.dex), ("LUK", mob.luk),
    ].iter()
        .filter(|(_, v)| v.is_some() && v.unwrap() > 1)
        .map(|(n, v)| format!("{}: {}", n, v.unwrap()))
        .collect();
    if !stats.is_empty() { parts.push(format!("Stats: {}", stats.join(", "))); }

    if let Some(c) = &mob.mob_class { parts.push(format!("Class: {}", c)); }

    parts.join(", ")
}

// ─── Qdrant Helpers ────────────────────────────────────────

async fn ensure_collection(client: &Client, qdrant_url: &str, name: &str, vector_size: u64) -> Result<()> {
    let url = format!("{}/collections/{}", qdrant_url, name);
    let resp = client.get(&url).send().await?;
    if resp.status().is_success() {
        info!("✅ Collection '{}' already exists, deleting for re-ingestion...", name);
        client.delete(&url).send().await?;
    }

    info!("🏗️ Creating collection '{}'", name);
    let body = json!({
        "vectors": {
            "size": vector_size,
            "distance": "Cosine"
        }
    });
    let resp = client.put(&url).json(&body).send().await?;
    if !resp.status().is_success() {
        let err = resp.text().await?;
        return Err(anyhow::anyhow!("Failed to create collection '{}': {}", name, err));
    }
    Ok(())
}

async fn upsert_batch(client: &Client, qdrant_url: &str, collection: &str, points: serde_json::Value) -> Result<()> {
    let url = format!("{}/collections/{}/points", qdrant_url, collection);
    let body = json!({ "points": points });
    let resp = client.put(&url).json(&body).send().await?;
    if !resp.status().is_success() {
        let err = resp.text().await?;
        return Err(anyhow::anyhow!("Upsert failed: {}", err));
    }
    Ok(())
}

// ─── Main ──────────────────────────────────────────────────

const BATCH_SIZE: usize = 50;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let ollama_url = env::var("OLLAMA_URL").unwrap_or_else(|_| "http://localhost:11434".to_string());
    let qdrant_url = env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6333".to_string());
    let embed_model = env::var("EMBED_MODEL").unwrap_or_else(|_| "nomic-embed-text".to_string());
    let rathena_db_path = env::var("RATHENA_DB_PATH").unwrap_or_else(|_| "../rathena/db/re".to_string());

    let client = Client::new();

    // Test Ollama connectivity & get vector size
    info!("🔌 Testing Ollama embedding model '{}'...", embed_model);
    let test_embeddings = get_embeddings(&client, &ollama_url, &embed_model, &["test".to_string()]).await?;
    let vector_size = test_embeddings[0].len() as u64;
    info!("✅ Embedding model ready, vector size: {}", vector_size);

    // ── Ingest Items ───────────────────────────────────────
    info!("📦 Loading item data from YAML files...");
    let item_files = ["item_db_equip.yml", "item_db_etc.yml", "item_db_usable.yml"];
    let mut all_items: Vec<YamlItem> = Vec::new();

    for file in &item_files {
        let path = format!("{}/{}", rathena_db_path, file);
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                match serde_yaml::from_str::<YamlItemRoot>(&content) {
                    Ok(root) => {
                        let count = root.body.as_ref().map(|b| b.len()).unwrap_or(0);
                        info!("  📄 {}: {} items", file, count);
                        if let Some(body) = root.body {
                            all_items.extend(body);
                        }
                    }
                    Err(e) => warn!("  ⚠️ Failed to parse {}: {}", file, e),
                }
            }
            Err(e) => warn!("  ⚠️ Failed to read {}: {}", file, e),
        }
    }

    info!("📦 Total items to ingest: {}", all_items.len());
    ensure_collection(&client, &qdrant_url, "ro_items", vector_size).await?;

    let mut ingested_items = 0;
    for batch in all_items.chunks(BATCH_SIZE) {
        let texts: Vec<String> = batch.iter().map(|i| item_to_text(i)).collect();

        match get_embeddings(&client, &ollama_url, &embed_model, &texts).await {
            Ok(embeddings) => {
                let points: Vec<serde_json::Value> = batch.iter().zip(embeddings.iter()).map(|(item, emb)| {
                    json!({
                        "id": item.id as u64,
                        "vector": emb,
                        "payload": {
                            "id": item.id,
                            "aegis_name": item.aegis_name,
                            "name": item.name,
                            "type": item.item_type,
                            "sub_type": item.sub_type,
                            "buy": item.buy,
                            "weight": item.weight,
                            "attack": item.attack,
                            "defense": item.defense,
                            "slots": item.slots,
                            "text": texts[batch.iter().position(|x| x.id == item.id).unwrap_or(0)]
                        }
                    })
                }).collect();

                if let Err(e) = upsert_batch(&client, &qdrant_url, "ro_items", json!(points)).await {
                    error!("❌ Failed to upsert item batch: {}", e);
                } else {
                    ingested_items += batch.len();
                    if ingested_items % 500 == 0 || ingested_items == all_items.len() {
                        info!("  ✅ Items: {}/{}", ingested_items, all_items.len());
                    }
                }
            }
            Err(e) => error!("❌ Embedding failed for item batch: {}", e),
        }
    }

    // ── Ingest Monsters ────────────────────────────────────
    info!("👹 Loading monster data from YAML...");
    let mob_path = format!("{}/mob_db.yml", rathena_db_path);
    let mob_content = std::fs::read_to_string(&mob_path)?;
    let mob_root: YamlMobRoot = serde_yaml::from_str(&mob_content)?;
    let all_mobs = mob_root.body.unwrap_or_default();
    info!("👹 Total monsters to ingest: {}", all_mobs.len());

    ensure_collection(&client, &qdrant_url, "ro_monsters", vector_size).await?;

    let mut ingested_mobs = 0;
    for batch in all_mobs.chunks(BATCH_SIZE) {
        let texts: Vec<String> = batch.iter().map(|m| mob_to_text(m)).collect();

        match get_embeddings(&client, &ollama_url, &embed_model, &texts).await {
            Ok(embeddings) => {
                let points: Vec<serde_json::Value> = batch.iter().zip(embeddings.iter()).map(|(mob, emb)| {
                    json!({
                        "id": mob.id as u64,
                        "vector": emb,
                        "payload": {
                            "id": mob.id,
                            "aegis_name": mob.aegis_name,
                            "name": mob.name,
                            "level": mob.level,
                            "hp": mob.hp,
                            "attack": mob.attack,
                            "defense": mob.defense,
                            "base_exp": mob.base_exp,
                            "job_exp": mob.job_exp,
                            "size": mob.size,
                            "race": mob.race,
                            "element": mob.element,
                            "class": mob.mob_class,
                            "text": texts[batch.iter().position(|x| x.id == mob.id).unwrap_or(0)]
                        }
                    })
                }).collect();

                if let Err(e) = upsert_batch(&client, &qdrant_url, "ro_monsters", json!(points)).await {
                    error!("❌ Failed to upsert mob batch: {}", e);
                } else {
                    ingested_mobs += batch.len();
                    if ingested_mobs % 500 == 0 || ingested_mobs == all_mobs.len() {
                        info!("  ✅ Monsters: {}/{}", ingested_mobs, all_mobs.len());
                    }
                }
            }
            Err(e) => error!("❌ Embedding failed for mob batch: {}", e),
        }
    }

    info!("🎉 Ingestion complete!");
    info!("  📦 Items: {}", ingested_items);
    info!("  👹 Monsters: {}", ingested_mobs);

    Ok(())
}
