//! RefGraph Neo4j persistence layer
//!
//! Provides Neo4j graph database integration for RefGraph.

pub mod config;
pub mod cypher;
pub mod service;

pub use config::Neo4jConfig;
pub use service::Neo4jService;
