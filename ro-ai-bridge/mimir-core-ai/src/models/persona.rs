use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{OnceLock, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Persona {
    pub name: String,
    pub display_name: String,
    pub tier: i8,
    pub model_id: Option<String>,
    pub system_prompt: String,
    pub greeting: Option<String>,
    pub allowed_actions: Vec<String>,
    pub personality_traits: Vec<String>,
}

/// Global cache for loaded personas
static PERSONA_CACHE: OnceLock<RwLock<HashMap<String, Persona>>> = OnceLock::new();

impl Persona {
    /// Get the base path for persona config files
    /// Can be configured via PERSONA_CONFIG_PATH environment variable
    fn get_base_path() -> String {
        std::env::var("PERSONA_CONFIG_PATH")
            .unwrap_or_else(|_| "config/personas".to_string())
    }

    /// Load a persona from a specific file path
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)?;
        let persona: Persona = serde_yaml::from_str(&content)?;
        
        // Validate required fields
        if persona.name.is_empty() {
            anyhow::bail!("Persona name cannot be empty");
        }
        if persona.system_prompt.is_empty() {
            anyhow::bail!("Persona system_prompt cannot be empty for '{}'", persona.name);
        }
        
        Ok(persona)
    }

    /// Load a persona by name from the default config directory
    pub fn load_by_name(name: &str) -> anyhow::Result<Self> {
        let base_path = Self::get_base_path();
        let path = format!("{}/{}.yaml", base_path, name);
        Self::load_from_file(path)
    }

    /// Load a persona by name with caching
    /// Subsequent calls will return the cached version
    pub fn load_by_name_cached(name: &str) -> anyhow::Result<Self> {
        let cache = PERSONA_CACHE.get_or_init(|| RwLock::new(HashMap::new()));
        
        // Try read lock first
        {
            let read = cache.read().map_err(|e| anyhow::anyhow!("Cache read lock error: {}", e))?;
            if let Some(persona) = read.get(name) {
                return Ok(persona.clone());
            }
        }
        
        // Load from file
        let persona = Self::load_by_name(name)?;
        
        // Write to cache
        {
            let mut write = cache.write().map_err(|e| anyhow::anyhow!("Cache write lock error: {}", e))?;
            write.insert(name.to_string(), persona.clone());
        }
        
        Ok(persona)
    }

    /// Clear the persona cache (useful for testing or hot-reloading)
    pub fn clear_cache() {
        if let Some(cache) = PERSONA_CACHE.get() {
            if let Ok(mut write) = cache.write() {
                write.clear();
            }
        }
    }
}
