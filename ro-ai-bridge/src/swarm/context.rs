use std::sync::Arc;
use tokio::sync::Mutex;
use serde::{Deserialize, Serialize};

/// Data shared across swarm agents and tools during a single request lifecycle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmState {
    pub tenant_id: String,
    pub original_query: String,
    pub accumulated_context: String,
    pub tools_called: Vec<String>,
    pub iteration_count: usize,
    pub max_iterations: usize,
    pub done: bool,
}

impl SwarmState {
    pub fn new(tenant_id: String, query: String, max_iters: usize) -> Self {
        Self {
            tenant_id,
            original_query: query,
            accumulated_context: String::new(),
            tools_called: Vec::new(),
            iteration_count: 0,
            max_iterations: max_iters,
            done: false,
        }
    }

    pub fn add_context(&mut self, source: &str, data: &str) {
        self.accumulated_context.push_str(&format!("\n--- [Source: {}] ---\n{}\n", source, data));
        self.tools_called.push(source.to_string());
    }

    pub fn increment_iteration(&mut self) -> Result<(), String> {
        self.iteration_count += 1;
        if self.iteration_count > self.max_iterations {
            self.done = true;
            return Err(format!("Max iterations ({}) reached. Swarm aborted to prevent infinite loop.", self.max_iterations));
        }
        Ok(())
    }
}

pub type SwarmContext = Arc<Mutex<SwarmState>>;

/// A guard that automatically tracks iterations and drops if stalled
pub struct IterationGuard {
    context: SwarmContext,
}

impl IterationGuard {
    pub fn new(context: SwarmContext) -> Self {
        Self { context }
    }

    pub async fn tick(&self) -> Result<(), String> {
        let mut state = self.context.lock().await;
        state.increment_iteration()
    }
}
