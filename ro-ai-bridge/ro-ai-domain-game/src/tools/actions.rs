use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;

// --- HEAL TOOL ---

#[derive(Debug)]
pub struct HealArgs {
    pub amount: u32,
}

impl<'de> Deserialize<'de> for HealArgs {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Parse into a generic JSON Value first to intercept LLM schema hallucinations (e.g. from llama3.2)
        if let Ok(v) = serde_json::Value::deserialize(deserializer) {
            let amount = match v.get("amount") {
                Some(serde_json::Value::Number(n)) => n.as_u64().unwrap_or(100) as u32,
                Some(serde_json::Value::String(s)) => s.parse::<u32>().unwrap_or(100),
                _ => 100, // Forgiving fallback for objects/maps/nulls/missing
            };
            Ok(HealArgs { amount })
        } else {
            // If the entire args block is unparsable, default to 100
            Ok(HealArgs { amount: 100 })
        }
    }
}

#[derive(Debug)]
pub struct HealError;

impl std::fmt::Display for HealError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HealTool Error")
    }
}

impl std::error::Error for HealError {}

#[derive(Serialize)]
pub struct HealResult {
    pub command: String,
    pub amount: u32,
}

pub struct HealTool {
    pub action_capture: Arc<Mutex<Option<serde_json::Value>>>,
}

impl HealTool {
    pub fn new(action_capture: Arc<Mutex<Option<serde_json::Value>>>) -> Self {
        Self { action_capture }
    }
}

impl Tool for HealTool {
    const NAME: &'static str = "heal";
    type Error = HealError;
    type Args = HealArgs;
    type Output = HealResult;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "heal".to_string(),
            description: "Casts a massive healing spell to restore the player's HP and SP. Use this when the player asks for healing, recovery, or mentions they are injured.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "amount": {
                        "type": "integer",
                        "description": "Amount to heal. Defaults to 100 for a full heal."
                    }
                },
                "required": ["amount"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let mut capture = self.action_capture.lock().await;
        *capture = Some(json!({
            "command": "heal",
            "params": {
                "amount": args.amount
            }
        }));

        Ok(HealResult {
            command: "heal".to_string(),
            amount: args.amount,
        })
    }
}

// --- BUFF TOOL ---

#[derive(Deserialize)]
pub struct BuffArgs {
    pub buff_type: String,
}

#[derive(Debug)]
pub struct BuffError;

impl std::fmt::Display for BuffError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BuffTool Error")
    }
}

impl std::error::Error for BuffError {}

#[derive(Serialize)]
pub struct BuffResult {
    pub command: String,
    pub buff_type: String,
}

pub struct BuffTool {
    pub action_capture: Arc<Mutex<Option<serde_json::Value>>>,
}

impl BuffTool {
    pub fn new(action_capture: Arc<Mutex<Option<serde_json::Value>>>) -> Self {
        Self { action_capture }
    }
}

impl Tool for BuffTool {
    const NAME: &'static str = "buff";
    type Error = BuffError;
    type Args = BuffArgs;
    type Output = BuffResult;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "buff".to_string(),
            description: "Casts supportive magic on the player. Use this when the player asks for buffs, blessing, agility, or enhancements.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "buff_type": {
                        "type": "string",
                        "description": "The requested buff type, e.g. 'blessing', 'agi', or 'all'"
                    }
                },
                "required": ["buff_type"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let mut capture = self.action_capture.lock().await;
        *capture = Some(json!({
            "command": "buff",
            "params": {
                "buff_type": args.buff_type.clone()
            }
        }));

        Ok(BuffResult {
            command: "buff".to_string(),
            buff_type: args.buff_type,
        })
    }
}
