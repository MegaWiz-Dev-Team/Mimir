//! Entity Extraction Service — Sprint 17
//!
//! Uses LLM to extract entities and relations from text chunks.
//! Outputs structured JSON for graph construction.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::warn;

// ═══════════════════════════════════════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════════════════════════════════════

/// Supported entity types for Knowledge Graph extraction.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntityType {
    Person,
    Organization,
    Location,
    Concept,
    Event,
    Product,
    Drug,
    Symptom,
    Item,
    Monster,
    Other,
}

impl EntityType {
    /// Parse entity type string (case-insensitive).
    pub fn from_str_flexible(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "person" | "people" | "character" => Self::Person,
            "organization" | "org" | "company" | "guild" => Self::Organization,
            "location" | "place" | "city" | "town" | "map" => Self::Location,
            "concept" | "idea" | "topic" | "skill" => Self::Concept,
            "event" | "quest" | "mission" | "battle" => Self::Event,
            "product" | "service" | "tool" => Self::Product,
            "drug" | "medicine" | "medication" | "pharmaceutical" => Self::Drug,
            "symptom" | "disease" | "condition" | "diagnosis" => Self::Symptom,
            "item" | "equipment" | "weapon" | "armor" | "card" => Self::Item,
            "monster" | "mob" | "enemy" | "creature" | "boss" | "npc" | "mvp" => Self::Monster,
            _ => Self::Other,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Person => "Person",
            Self::Organization => "Organization",
            Self::Location => "Location",
            Self::Concept => "Concept",
            Self::Event => "Event",
            Self::Product => "Product",
            Self::Drug => "Drug",
            Self::Symptom => "Symptom",
            Self::Item => "Item",
            Self::Monster => "Monster",
            Self::Other => "Other",
        }
    }
}

/// An extracted entity from text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedEntity {
    pub name: String,
    #[serde(rename = "type", alias = "entity_type")]
    pub entity_type: String,
    #[serde(default)]
    pub properties: Option<Value>,
}

/// An extracted relation from text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedRelation {
    pub from: String,
    pub to: String,
    #[serde(rename = "type")]
    pub relation_type: String,
    #[serde(default)]
    pub properties: Option<Value>,
}

/// Result of entity extraction from a single chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionResult {
    pub entities: Vec<ExtractedEntity>,
    pub relations: Vec<ExtractedRelation>,
}

impl Default for ExtractionResult {
    fn default() -> Self {
        Self {
            entities: vec![],
            relations: vec![],
        }
    }
}

/// Extraction run status tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionRunStatus {
    pub entities_found: usize,
    pub relations_found: usize,
    pub chunks_processed: usize,
    pub chunks_failed: usize,
    pub status: String,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Prompt Construction (Pure functions — testable without LLM)
// ═══════════════════════════════════════════════════════════════════════════════

/// Build the system prompt for entity extraction.
pub fn build_extraction_system_prompt() -> String {
    r#"You are a knowledge graph extraction engine. Your task is to extract entities and their relationships from the given text.

Output ONLY valid JSON in this exact format:
{
  "entities": [
    {"name": "EntityName", "type": "EntityType", "properties": {"key": "value"}}
  ],
  "relations": [
    {"from": "EntityA", "to": "EntityB", "type": "relationship_type"}
  ]
}

Entity types to extract:
- Person: people, characters, NPCs
- Organization: companies, guilds, groups
- Location: places, cities, maps, dungeons
- Concept: ideas, skills, topics, mechanics
- Event: quests, missions, battles, events
- Product: services, tools, applications
- Drug: medicines, medications, pharmaceuticals
- Symptom: diseases, conditions, diagnoses
- Item: equipment, weapons, armor, cards
- Monster: mobs, enemies, creatures, bosses, MVPs

Rules:
1. Extract ALL meaningful entities and their relationships
2. Use consistent naming (capitalize properly)
3. Keep entity names concise but descriptive
4. Use descriptive relationship types (e.g., "treats", "located_in", "drops", "belongs_to")
5. If no entities found, return {"entities": [], "relations": []}
6. Do NOT include explanations — ONLY the JSON"#
        .to_string()
}

/// Build the user prompt with the text chunk.
pub fn build_extraction_user_prompt(text: &str, max_entities: usize) -> String {
    format!(
        "Extract up to {} entities and their relationships from this text:\n\n---\n{}\n---",
        max_entities, text
    )
}

// ═══════════════════════════════════════════════════════════════════════════════
// JSON Parsing (Pure functions — testable without LLM)
// ═══════════════════════════════════════════════════════════════════════════════

/// Parse LLM response into ExtractionResult.
/// Handles valid JSON, partial JSON, and wrapped JSON (```json blocks).
pub fn parse_extraction_response(response: &str) -> ExtractionResult {
    let cleaned = clean_json_response(response);

    match serde_json::from_str::<ExtractionResult>(&cleaned) {
        Ok(result) => result,
        Err(e) => {
            warn!(
                "Failed to parse extraction response: {}. Trying fallback.",
                e
            );
            // Try to extract entities array only
            if let Ok(val) = serde_json::from_str::<Value>(&cleaned) {
                return extraction_from_value(&val);
            }
            warn!("Fallback parsing also failed. Returning empty result.");
            ExtractionResult::default()
        }
    }
}

/// Clean LLM response — strip markdown code blocks and extra text.
pub fn clean_json_response(response: &str) -> String {
    let trimmed = response.trim();

    // Strip ```json ... ``` blocks
    if let Some(start) = trimmed.find("```json") {
        let after_marker = &trimmed[start + 7..];
        if let Some(end) = after_marker.find("```") {
            return after_marker[..end].trim().to_string();
        }
    }

    // Strip ``` ... ``` blocks
    if let Some(start) = trimmed.find("```") {
        let after_marker = &trimmed[start + 3..];
        if let Some(end) = after_marker.find("```") {
            return after_marker[..end].trim().to_string();
        }
    }

    // Find first { and last }
    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if start <= end {
            return trimmed[start..=end].to_string();
        }
    }

    trimmed.to_string()
}

/// Try to extract entities/relations from a serde_json::Value.
fn extraction_from_value(val: &Value) -> ExtractionResult {
    let entities = if let Some(arr) = val.get("entities").and_then(|v| v.as_array()) {
        arr.iter()
            .filter_map(|e| serde_json::from_value::<ExtractedEntity>(e.clone()).ok())
            .collect()
    } else {
        vec![]
    };

    let relations = if let Some(arr) = val.get("relations").and_then(|v| v.as_array()) {
        arr.iter()
            .filter_map(|r| serde_json::from_value::<ExtractedRelation>(r.clone()).ok())
            .collect()
    } else {
        vec![]
    };

    ExtractionResult {
        entities,
        relations,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Deduplication (Pure functions — testable without DB)
// ═══════════════════════════════════════════════════════════════════════════════

/// Deduplicate entities by name + type (case-insensitive).
/// Merges properties from duplicates.
pub fn dedup_entities(entities: Vec<ExtractedEntity>) -> Vec<ExtractedEntity> {
    use std::collections::HashMap;

    let mut seen: HashMap<String, ExtractedEntity> = HashMap::new();

    for entity in entities {
        let key = format!(
            "{}::{}",
            entity.name.to_lowercase(),
            entity.entity_type.to_lowercase()
        );

        if let Some(existing) = seen.get_mut(&key) {
            // Merge properties if the new one has them
            if let Some(new_props) = &entity.properties {
                if let Some(existing_props) = &mut existing.properties {
                    if let (Some(existing_obj), Some(new_obj)) =
                        (existing_props.as_object_mut(), new_props.as_object())
                    {
                        for (k, v) in new_obj {
                            existing_obj.insert(k.clone(), v.clone());
                        }
                    }
                } else {
                    existing.properties = Some(new_props.clone());
                }
            }
        } else {
            // Normalize entity type
            let normalized_type = EntityType::from_str_flexible(&entity.entity_type);
            seen.insert(
                key,
                ExtractedEntity {
                    name: entity.name,
                    entity_type: normalized_type.as_str().to_string(),
                    properties: entity.properties,
                },
            );
        }
    }

    seen.into_values().collect()
}

/// Deduplicate relations by from+to+type (case-insensitive).
pub fn dedup_relations(relations: Vec<ExtractedRelation>) -> Vec<ExtractedRelation> {
    use std::collections::HashSet;

    let mut seen: HashSet<String> = HashSet::new();
    let mut result = Vec::new();

    for rel in relations {
        let key = format!(
            "{}::{}::{}",
            rel.from.to_lowercase(),
            rel.to.to_lowercase(),
            rel.relation_type.to_lowercase()
        );

        if seen.insert(key) {
            result.push(rel);
        }
    }

    result
}

// ═══════════════════════════════════════════════════════════════════════════════
// TDD Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ========================================
    // UT-017o: System prompt construction
    // ========================================
    #[test]
    fn test_system_prompt_contains_entity_types() {
        let prompt = build_extraction_system_prompt();
        assert!(prompt.contains("Person"), "Prompt must mention Person type");
        assert!(
            prompt.contains("Organization"),
            "Prompt must mention Organization"
        );
        assert!(prompt.contains("Drug"), "Prompt must mention Drug");
        assert!(prompt.contains("Monster"), "Prompt must mention Monster");
        assert!(prompt.contains("Item"), "Prompt must mention Item");
        assert!(
            prompt.contains("JSON"),
            "Prompt must mention JSON output format"
        );
    }

    // ========================================
    // UT-017p: User prompt construction
    // ========================================
    #[test]
    fn test_user_prompt_includes_text() {
        let prompt = build_extraction_user_prompt("Some text about Aspirin", 20);
        assert!(prompt.contains("Some text about Aspirin"));
        assert!(prompt.contains("20"));
    }

    // ========================================
    // UT-017q: Parse valid JSON response
    // ========================================
    #[test]
    fn test_parse_valid_json_response() {
        let response = r#"{
            "entities": [
                {"name": "Aspirin", "type": "Drug", "properties": {"category": "NSAID"}},
                {"name": "Headache", "type": "Symptom"}
            ],
            "relations": [
                {"from": "Aspirin", "to": "Headache", "type": "treats"}
            ]
        }"#;

        let result = parse_extraction_response(response);
        assert_eq!(result.entities.len(), 2);
        assert_eq!(result.relations.len(), 1);
        assert_eq!(result.entities[0].name, "Aspirin");
        assert_eq!(result.entities[0].entity_type, "Drug");
        assert_eq!(result.relations[0].relation_type, "treats");
    }

    // ========================================
    // UT-017r: Parse JSON wrapped in markdown code block
    // ========================================
    #[test]
    fn test_parse_json_in_code_block() {
        let response = r#"Here is the result:
```json
{
    "entities": [{"name": "Poring", "type": "Monster"}],
    "relations": []
}
```
That's all."#;

        let result = parse_extraction_response(response);
        assert_eq!(result.entities.len(), 1);
        assert_eq!(result.entities[0].name, "Poring");
    }

    // ========================================
    // UT-017s: Parse invalid JSON returns empty
    // ========================================
    #[test]
    fn test_parse_invalid_json_returns_empty() {
        let response = "This is not JSON at all, just some random text.";
        let result = parse_extraction_response(response);
        assert!(result.entities.is_empty());
        assert!(result.relations.is_empty());
    }

    // ========================================
    // UT-017t: Parse partial JSON — entities only
    // ========================================
    #[test]
    fn test_parse_partial_json_entities_only() {
        let response = r#"{"entities": [{"name": "Prontera", "type": "Location"}]}"#;
        let result = parse_extraction_response(response);
        assert_eq!(result.entities.len(), 1);
        assert_eq!(result.entities[0].name, "Prontera");
        assert!(result.relations.is_empty());
    }

    // ========================================
    // UT-017u: Dedup entities — case insensitive
    // ========================================
    #[test]
    fn test_dedup_entities_case_insensitive() {
        let entities = vec![
            ExtractedEntity {
                name: "Aspirin".to_string(),
                entity_type: "Drug".to_string(),
                properties: Some(json!({"category": "NSAID"})),
            },
            ExtractedEntity {
                name: "aspirin".to_string(),
                entity_type: "drug".to_string(),
                properties: Some(json!({"dosage": "500mg"})),
            },
            ExtractedEntity {
                name: "Headache".to_string(),
                entity_type: "Symptom".to_string(),
                properties: None,
            },
        ];

        let deduped = dedup_entities(entities);
        assert_eq!(
            deduped.len(),
            2,
            "Should merge duplicates (Aspirin + aspirin = 1)"
        );

        // Find the Aspirin entity (after dedup)
        let aspirin = deduped
            .iter()
            .find(|e| e.name.to_lowercase() == "aspirin")
            .unwrap();
        // Properties should be merged
        let props = aspirin.properties.as_ref().unwrap();
        assert!(
            props.get("category").is_some() || props.get("dosage").is_some(),
            "Should have merged properties"
        );
    }

    // ========================================
    // UT-017v: Dedup relations
    // ========================================
    #[test]
    fn test_dedup_relations() {
        let relations = vec![
            ExtractedRelation {
                from: "Aspirin".to_string(),
                to: "Headache".to_string(),
                relation_type: "treats".to_string(),
                properties: None,
            },
            ExtractedRelation {
                from: "aspirin".to_string(),
                to: "headache".to_string(),
                relation_type: "Treats".to_string(),
                properties: None,
            },
        ];

        let deduped = dedup_relations(relations);
        assert_eq!(deduped.len(), 1, "Should deduplicate case-insensitive");
    }

    // ========================================
    // UT-017w: Entity type parsing — flexible
    // ========================================
    #[test]
    fn test_entity_type_parsing() {
        assert_eq!(EntityType::from_str_flexible("Person"), EntityType::Person);
        assert_eq!(
            EntityType::from_str_flexible("character"),
            EntityType::Person
        );
        assert_eq!(
            EntityType::from_str_flexible("guild"),
            EntityType::Organization
        );
        assert_eq!(EntityType::from_str_flexible("mob"), EntityType::Monster);
        assert_eq!(EntityType::from_str_flexible("boss"), EntityType::Monster);
        assert_eq!(EntityType::from_str_flexible("weapon"), EntityType::Item);
        assert_eq!(EntityType::from_str_flexible("medicine"), EntityType::Drug);
        assert_eq!(
            EntityType::from_str_flexible("disease"),
            EntityType::Symptom
        );
        assert_eq!(EntityType::from_str_flexible("quest"), EntityType::Event);
        assert_eq!(EntityType::from_str_flexible("map"), EntityType::Location);
        assert_eq!(
            EntityType::from_str_flexible("unknown_xyz"),
            EntityType::Other
        );
    }

    // ========================================
    // UT-017x: Empty extraction result
    // ========================================
    #[test]
    fn test_empty_extraction_result() {
        let response = r#"{"entities": [], "relations": []}"#;
        let result = parse_extraction_response(response);
        assert!(result.entities.is_empty());
        assert!(result.relations.is_empty());
    }

    // ========================================
    // UT-017y: Clean JSON response — strip extra text
    // ========================================
    #[test]
    fn test_clean_json_response() {
        // Test stripping extra text before/after JSON
        let input = "Here is the result: {\"entities\": []} some trailing text";
        let cleaned = clean_json_response(input);
        assert_eq!(cleaned, r#"{"entities": []}"#);

        // Test code block stripping
        let input2 = "```json\n{\"entities\": []}\n```";
        let cleaned2 = clean_json_response(input2);
        assert_eq!(cleaned2, r#"{"entities": []}"#);
    }

    // ========================================
    // UT-017z: ExtractionResult default
    // ========================================
    #[test]
    fn test_extraction_result_default() {
        let result = ExtractionResult::default();
        assert!(result.entities.is_empty());
        assert!(result.relations.is_empty());
    }

    // ========================================
    // UT-017za: Extraction run status serialization
    // ========================================
    #[test]
    fn test_extraction_run_status() {
        let status = ExtractionRunStatus {
            entities_found: 10,
            relations_found: 5,
            chunks_processed: 3,
            chunks_failed: 0,
            status: "completed".to_string(),
        };
        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json["entities_found"], 10);
        assert_eq!(json["relations_found"], 5);
        assert_eq!(json["status"], "completed");
    }
}
