use anyhow::Result;
use mimir_core_ai::rag_engine::DynamicContextPlugin;
use mimir_core_ai::services::db::DbPool;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Mob data from rAthena database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct MobData {
    pub id: u32,
    pub name_aegis: String,
    pub name_english: String,
    pub level: u32,
    pub hp: u64,
    pub sp: u32,
    pub base_exp: u64,
    pub job_exp: u64,
    pub attack: u32,
    pub defense: u32,
    pub magic_defense: u32,
    pub str: u32,
    pub agi: u32,
    pub vit: u32,
    pub int: u32,
    pub dex: u32,
    pub luk: u32,
    pub size: String,
    pub race: String,
    pub element: String,
    pub element_level: u32,
}

/// Item data from rAthena database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ItemData {
    pub id: u32,
    pub name_aegis: String,
    pub name_english: String,
    pub item_type: String,
    pub subtype: Option<String>,
    pub attack: u32,
    pub magic_attack: u32,
    pub defense: u32,
    pub weight: u32,
    pub slots: u32,
    pub weapon_level: u32,
    pub armor_level: u32,
    pub equip_level_min: u32,
    pub price_buy: u64,
    pub price_sell: u64,
    pub refineable: bool,
}

// ─── Custom Tools ──────────────────────────────────────────────────────────

/// Tool for querying mob database
pub struct QueryMobDbTool {
    db_pool: DbPool,
}

impl QueryMobDbTool {
    pub fn new(db_pool: DbPool) -> Self {
        Self { db_pool }
    }

    /// Query mob by name (partial match)
    pub async fn query_by_name(&self, name: &str) -> Result<Vec<MobData>> {
        let pattern = format!("%{}%", name);
        let mobs = sqlx::query_as::<_, MobData>(
            r#"SELECT 
                id, name_aegis, name_english, level, hp, sp,
                base_exp, job_exp, attack, defense, magic_defense,
                str, agi, vit, int, dex, luk,
                size, race, element, element_level
            FROM mob_db 
            WHERE name_english LIKE ? OR name_aegis LIKE ?
            LIMIT 10"#,
        )
        .bind(&pattern)
        .bind(&pattern)
        .fetch_all(&self.db_pool)
        .await?;

        Ok(mobs)
    }

    /// Query mob by ID
    pub async fn query_by_id(&self, id: u32) -> Result<Option<MobData>> {
        let mob = sqlx::query_as::<_, MobData>(
            r#"SELECT 
                id, name_aegis, name_english, level, hp, sp,
                base_exp, job_exp, attack, defense, magic_defense,
                str, agi, vit, int, dex, luk,
                size, race, element, element_level
            FROM mob_db WHERE id = ?"#,
        )
        .bind(id)
        .fetch_optional(&self.db_pool)
        .await?;

        Ok(mob)
    }

    /// Query mobs by level range
    pub async fn query_by_level_range(
        &self,
        min_level: u32,
        max_level: u32,
    ) -> Result<Vec<MobData>> {
        let mobs = sqlx::query_as::<_, MobData>(
            r#"SELECT 
                id, name_aegis, name_english, level, hp, sp,
                base_exp, job_exp, attack, defense, magic_defense,
                str, agi, vit, int, dex, luk,
                size, race, element, element_level
            FROM mob_db 
            WHERE level BETWEEN ? AND ?
            ORDER BY level, base_exp DESC
            LIMIT 20"#,
        )
        .bind(min_level)
        .bind(max_level)
        .fetch_all(&self.db_pool)
        .await?;

        Ok(mobs)
    }

    /// Format mob data as human-readable text
    pub fn format_mob(mob: &MobData) -> String {
        format!(
            "**{}** (ID: {})\n\
            - Level: {} | HP: {} | SP: {}\n\
            - ATK: {} | DEF: {} | MDEF: {}\n\
            - EXP: Base {} / Job {}\n\
            - Size: {} | Race: {} | Element: {} Lv{}\n\
            - Stats: STR {} AGI {} VIT {} INT {} DEX {} LUK {}",
            mob.name_english,
            mob.id,
            mob.level,
            mob.hp,
            mob.sp,
            mob.attack,
            mob.defense,
            mob.magic_defense,
            mob.base_exp,
            mob.job_exp,
            mob.size,
            mob.race,
            mob.element,
            mob.element_level,
            mob.str,
            mob.agi,
            mob.vit,
            mob.int,
            mob.dex,
            mob.luk
        )
    }
}

/// Tool for querying item database
pub struct QueryItemDbTool {
    db_pool: DbPool,
}

impl QueryItemDbTool {
    pub fn new(db_pool: DbPool) -> Self {
        Self { db_pool }
    }

    /// Query item by name (partial match)
    pub async fn query_by_name(&self, name: &str) -> Result<Vec<ItemData>> {
        let pattern = format!("%{}%", name);
        let items = sqlx::query_as::<_, ItemData>(
            r#"SELECT 
                id, name_aegis, name_english, item_type, subtype,
                attack, magic_attack, defense, weight, slots,
                weapon_level, armor_level, equip_level_min,
                price_buy, price_sell, refineable
            FROM item_db 
            WHERE name_english LIKE ? OR name_aegis LIKE ?
            LIMIT 10"#,
        )
        .bind(&pattern)
        .bind(&pattern)
        .fetch_all(&self.db_pool)
        .await?;

        Ok(items)
    }

    /// Query item by ID
    pub async fn query_by_id(&self, id: u32) -> Result<Option<ItemData>> {
        let item = sqlx::query_as::<_, ItemData>(
            r#"SELECT 
                id, name_aegis, name_english, item_type, subtype,
                attack, magic_attack, defense, weight, slots,
                weapon_level, armor_level, equip_level_min,
                price_buy, price_sell, refineable
            FROM item_db WHERE id = ?"#,
        )
        .bind(id)
        .fetch_optional(&self.db_pool)
        .await?;

        Ok(item)
    }

    /// Query items by type
    pub async fn query_by_type(&self, item_type: &str) -> Result<Vec<ItemData>> {
        let items = sqlx::query_as::<_, ItemData>(
            r#"SELECT 
                id, name_aegis, name_english, item_type, subtype,
                attack, magic_attack, defense, weight, slots,
                weapon_level, armor_level, equip_level_min,
                price_buy, price_sell, refineable
            FROM item_db 
            WHERE item_type = ?
            ORDER BY name_english
            LIMIT 50"#,
        )
        .bind(item_type)
        .fetch_all(&self.db_pool)
        .await?;

        Ok(items)
    }

    /// Format item data as human-readable text
    pub fn format_item(item: &ItemData) -> String {
        let mut parts = vec![format!("**{}** (ID: {})", item.name_english, item.id)];

        parts.push(format!("- Type: {}", item.item_type));

        if let Some(ref subtype) = item.subtype {
            parts.push(format!("  Subtype: {}", subtype));
        }

        if item.attack > 0 {
            parts.push(format!("- ATK: {}", item.attack));
        }
        if item.magic_attack > 0 {
            parts.push(format!("- MATK: {}", item.magic_attack));
        }
        if item.defense > 0 {
            parts.push(format!("- DEF: {}", item.defense));
        }

        parts.push(format!("- Weight: {:.1}", item.weight as f32 / 10.0));

        if item.slots > 0 {
            parts.push(format!("- Slots: {}", item.slots));
        }

        if item.price_buy > 0 {
            parts.push(format!("- Buy Price: {} currency", item.price_buy));
        }
        if item.price_sell > 0 {
            parts.push(format!("- Sell Price: {} currency", item.price_sell));
        }

        if item.weapon_level > 0 {
            parts.push(format!("- Weapon Level: {}", item.weapon_level));
        }
        if item.armor_level > 0 {
            parts.push(format!("- Armor Level: {}", item.armor_level));
        }
        if item.equip_level_min > 0 {
            parts.push(format!("- Required Level: {}", item.equip_level_min));
        }

        if item.refineable {
            parts.push("- Refineable: Yes".to_string());
        }

        parts.join("\n")
    }
}
use regex::Regex;

#[async_trait::async_trait]
impl DynamicContextPlugin for QueryMobDbTool {
    async fn get_context<'a>(
        &'a self,
        message: &'a str,
        tools_used: &'a mut Vec<String>,
    ) -> Result<String> {
        let msg_lower = message.to_lowercase();

        let is_mob_query = msg_lower.contains("mob")
            || msg_lower.contains("monster")
            || msg_lower.contains("boss")
            || msg_lower.contains("mvp");

        if !is_mob_query {
            return Ok(String::new());
        }

        let entity_name = extract_entity_name(message);
        let mut context = String::new();

        if let Some(name) = entity_name {
            if let Ok(mobs) = self.query_by_name(&name).await {
                if !mobs.is_empty() {
                    tools_used.push(format!("query_mob:{}", name));
                    context.push_str("=== Mob Database Information ===\n");
                    for mob in mobs.iter().take(3) {
                        context.push_str(&Self::format_mob(mob));
                        context.push_str("\n\n");
                    }
                }
            }
        }
        Ok(context)
    }
}

#[async_trait::async_trait]
impl DynamicContextPlugin for QueryItemDbTool {
    async fn get_context<'a>(
        &'a self,
        message: &'a str,
        tools_used: &'a mut Vec<String>,
    ) -> Result<String> {
        let msg_lower = message.to_lowercase();

        let is_item_query = msg_lower.contains("item")
            || msg_lower.contains("weapon")
            || msg_lower.contains("armor")
            || msg_lower.contains("card")
            || msg_lower.contains("gear")
            || msg_lower.contains("equip");

        if !is_item_query {
            return Ok(String::new());
        }

        let entity_name = extract_entity_name(message);
        let mut context = String::new();

        if let Some(name) = entity_name {
            if let Ok(items) = self.query_by_name(&name).await {
                if !items.is_empty() {
                    tools_used.push(format!("query_item:{}", name));
                    context.push_str("=== Item Database Information ===\n");
                    for item in items.iter().take(3) {
                        context.push_str(&Self::format_item(item));
                        context.push_str("\n\n");
                    }
                }
            }
        }
        Ok(context)
    }
}

/// Helper to extract capitalized entity names from the query
pub fn extract_entity_name(query: &str) -> Option<String> {
    let re = Regex::new(r"\b[A-Z][a-zA-Z]*\b").unwrap();
    let words: Vec<&str> = re.find_iter(query).map(|m| m.as_str()).collect();

    // Filter out common sentence starters that might be capitalized
    let ignored = vec![
        "What", "How", "Where", "When", "Why", "Who", "Tell", "Can", "Is", "Are",
    ];

    let filtered: Vec<&str> = words.into_iter().filter(|w| !ignored.contains(w)).collect();

    if !filtered.is_empty() {
        Some(filtered.join(" "))
    } else {
        None
    }
}
