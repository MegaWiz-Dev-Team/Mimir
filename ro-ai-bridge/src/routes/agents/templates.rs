//! Agent Templates — predefined agent configurations.

use axum::Json;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct AgentTemplate {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub system_prompt: String,
    pub model_id: String,
    pub provider: String,
    pub temperature: f64,
    pub max_tokens: i32,
    pub use_rag: bool,
    pub use_knowledge_graph: bool,
    pub tools: Vec<String>,
    pub personality_traits: Vec<String>,
    pub greeting: String,
    pub tier: i32,
    pub avatar_url: String,
}

fn get_templates() -> Vec<AgentTemplate> {
    vec![
        // ─── NPC Game Agent (consolidated) ──────────────────────────────
        AgentTemplate {
            id: "npc_game_agent".into(),
            name: "npc_game_agent".into(),
            display_name: "NPC Game Agent".into(),
            description: "Ragnarok Online NPC with RAG knowledge retrieval and action commands".into(),
            system_prompt: "คุณคือ NPC ในเกม Ragnarok Online สามารถช่วยตอบคำถาม ค้นหาข้อมูล Monster, Item, Map จาก Knowledge Base (RAG) และดำเนินการคำสั่ง (Action) เช่น Heal, Buff, Warp ให้ผู้เล่นได้ ตอบเป็นภาษาไทยเสมอ อธิบายอย่างเป็นมิตรและกระชับ".into(),
            model_id: "mlx-community/Qwen3.5-35B-A3B-4bit".into(),
            provider: "heimdall".into(),
            temperature: 0.7,
            max_tokens: 4096,
            use_rag: true,
            use_knowledge_graph: false,
            tools: vec!["QueryMobDb".into(), "QueryItemDb".into(), "heal".into(), "buff".into(), "warp".into()],
            personality_traits: vec!["helpful".into(), "wise".into(), "friendly".into()],
            greeting: "สวัสดีนักผจญภัย! ข้าพร้อมช่วยเหลือท่าน ไม่ว่าจะค้นหาข้อมูล Monster, Item หรือสั่ง Heal/Buff/Warp ได้เลย!".into(),
            tier: 2,
            avatar_url: "/avatars/mimir.png".into(),
        },
        // ─── Medical Doctor ─────────────────────────────────────────────
        AgentTemplate {
            id: "medical_doctor".into(),
            name: "medical_doctor".into(),
            display_name: "Medical Doctor".into(),
            description: "AI medical assistant for health Q&A with RAG-powered clinical knowledge".into(),
            system_prompt: "You are a medical AI assistant trained to provide general health information and answer medical questions. Use RAG to retrieve evidence-based medical knowledge from clinical databases. Always provide disclaimers that you are not a substitute for professional medical advice. Answer clearly and accurately, citing sources when possible. Support both Thai and English.".into(),
            model_id: "lmstudio-community/medgemma-4b-it-MLX-4bit".into(),
            provider: "heimdall".into(),
            temperature: 0.3,
            max_tokens: 4096,
            use_rag: true,
            use_knowledge_graph: true,
            tools: vec!["WebSearch".into()],
            personality_traits: vec!["precise".into(), "empathetic".into(), "analytical".into(), "thorough".into()],
            greeting: "สวัสดีครับ ผมเป็น AI Medical Assistant พร้อมให้คำปรึกษาด้านสุขภาพเบื้องต้น กรุณาสอบถามได้เลยครับ\n\n⚠️ **หมายเหตุ:** ข้อมูลที่ให้เป็นเพียงข้อมูลทั่วไป ไม่ใช่การวินิจฉัยหรือรักษาโรค กรุณาปรึกษาแพทย์สำหรับปัญหาสุขภาพเฉพาะทาง".into(),
            tier: 2,
            avatar_url: "/avatars/medical.png".into(),
        },
        // ─── Data Analytics ─────────────────────────────────────────────
        AgentTemplate {
            id: "data_analytics".into(),
            name: "data_analytics".into(),
            display_name: "Data Analytics".into(),
            description: "Data analysis agent for SQL queries, statistical insights, and report generation".into(),
            system_prompt: "You are a Data Analytics AI assistant specialized in data analysis, SQL query generation, statistical analysis, and business intelligence. Help users explore datasets, write SQL queries, interpret results, create visualizations descriptions, and generate actionable insights. Support both Thai and English. Present findings in structured, easy-to-understand formats with tables and bullet points.".into(),
            model_id: "mlx-community/Qwen3.5-35B-A3B-4bit".into(),
            provider: "heimdall".into(),
            temperature: 0.4,
            max_tokens: 4096,
            use_rag: true,
            use_knowledge_graph: false,
            tools: vec!["Calculator".into(), "WebSearch".into()],
            personality_traits: vec!["analytical".into(), "precise".into(), "structured".into(), "insightful".into()],
            greeting: "สวัสดีครับ ผมเป็น Data Analytics Assistant พร้อมช่วยวิเคราะห์ข้อมูล เขียน SQL Query และสร้าง Insights จากข้อมูลของคุณครับ\n\n**ตัวอย่างสิ่งที่ช่วยได้:**\n- เขียน SQL Query จากคำอธิบาย\n- วิเคราะห์แนวโน้มข้อมูล\n- สรุป KPI และ Metrics\n- สร้างรายงานจากข้อมูลดิบ".into(),
            tier: 2,
            avatar_url: "/avatars/data_analyst.png".into(),
        },
        // ─── Customer Support ───────────────────────────────────────────
        AgentTemplate {
            id: "customer_support".into(),
            name: "customer_support".into(),
            display_name: "Customer Support".into(),
            description: "Polite customer service agent with FAQ knowledge and ticket handling".into(),
            system_prompt: "You are a Customer Support AI assistant. Help users resolve issues, answer frequently asked questions, and provide excellent service. Use RAG to retrieve relevant FAQ and knowledge base articles. Be polite, patient, and solution-oriented. Escalate complex issues when necessary. Support both Thai and English.".into(),
            model_id: "mlx-community/Qwen3.5-35B-A3B-4bit".into(),
            provider: "heimdall".into(),
            temperature: 0.5,
            max_tokens: 2048,
            use_rag: true,
            use_knowledge_graph: false,
            tools: vec![],
            personality_traits: vec!["friendly".into(), "patient".into(), "empathetic".into(), "helpful".into()],
            greeting: "สวัสดีครับ ยินดีให้บริการ! มีอะไรให้ช่วยเหลือครับ? 😊\n\nผมสามารถช่วยตอบคำถาม แก้ปัญหา หรือแนะนำข้อมูลได้เลยครับ".into(),
            tier: 2,
            avatar_url: "/avatars/support.png".into(),
        },
    ]
}

/// GET /api/v1/agents/templates — List predefined templates
pub(crate) async fn list_templates() -> Json<Vec<AgentTemplate>> {
    Json(get_templates())
}

// ─── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// TC_MIG_01: Templates are valid and contain expected categories
    #[test]
    fn test_templates_are_valid() {
        let templates = get_templates();
        assert_eq!(templates.len(), 4, "Should have 4 templates");
        assert_eq!(templates[0].id, "npc_game_agent");
        assert_eq!(templates[1].id, "medical_doctor");
        assert_eq!(templates[2].id, "data_analytics");
        assert_eq!(templates[3].id, "customer_support");
    }

    /// TC_MIG_02: All templates have required fields populated
    #[test]
    fn test_templates_have_required_fields() {
        for t in &get_templates() {
            assert!(!t.name.is_empty(), "Template name must not be empty");
            assert!(
                !t.system_prompt.is_empty(),
                "System prompt must not be empty"
            );
            assert!(!t.model_id.is_empty(), "Model ID must not be empty");
            assert!(!t.greeting.is_empty(), "Greeting must not be empty");
            assert!(!t.provider.is_empty(), "Provider must not be empty");
            assert!(!t.avatar_url.is_empty(), "Avatar URL must not be empty");
            assert!(!t.personality_traits.is_empty(), "Traits must not be empty");
        }
    }

    /// TC_MIG_03: All templates are Tier 2
    #[test]
    fn test_template_tiers() {
        let templates = get_templates();
        let tier2_count = templates.iter().filter(|t| t.tier == 2).count();
        assert_eq!(tier2_count, 4, "All 4 templates should be Tier 2");
    }

    /// TC_MIG_04: All templates use RAG
    #[test]
    fn test_all_templates_use_rag() {
        let templates = get_templates();
        for t in &templates {
            assert!(t.use_rag, "Template '{}' should use RAG", t.id);
        }
    }

    /// TC_MIG_05: NPC game agent has action tools + RAG tools
    #[test]
    fn test_npc_agent_has_action_tools() {
        let templates = get_templates();
        let npc = templates
            .iter()
            .find(|t| t.id == "npc_game_agent")
            .expect("NPC game agent");
        assert!(
            npc.tools.contains(&"heal".to_string()),
            "NPC should have heal"
        );
        assert!(
            npc.tools.contains(&"buff".to_string()),
            "NPC should have buff"
        );
        assert!(
            npc.tools.contains(&"warp".to_string()),
            "NPC should have warp"
        );
        assert!(
            npc.tools.contains(&"QueryMobDb".to_string()),
            "NPC should have QueryMobDb"
        );
    }

    /// TC_MIG_05b: Medical Doctor uses MedGemma and knowledge graph
    #[test]
    fn test_medical_doctor_config() {
        let templates = get_templates();
        let med = templates
            .iter()
            .find(|t| t.id == "medical_doctor")
            .expect("Medical Doctor");
        assert!(
            med.model_id.contains("medgemma"),
            "Medical Doctor should use MedGemma model"
        );
        assert!(
            med.use_knowledge_graph,
            "Medical Doctor should use knowledge graph"
        );
        assert!(
            med.temperature <= 0.4,
            "Medical Doctor should have low temperature for accuracy"
        );
    }

    /// TC_MIG_06: All NPC templates use Heimdall provider
    #[test]
    fn test_npc_templates_use_heimdall() {
        for t in &get_templates() {
            assert_eq!(
                t.provider, "heimdall",
                "NPC template '{}' should use heimdall",
                t.id
            );
        }
    }
}
