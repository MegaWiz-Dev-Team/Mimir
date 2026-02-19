# 🔌 rAthena Client-to-AI Agent Communication Methods

เอกสารฉบับนี้อธิบายวิธีการที่ **Game Client** สื่อสารกับ **rAthena Server** และต่อไปยัง **AI Agent** โดยละเอียดในระดับที่สามารถ implement ได้จริง

---

## 📊 System Communication Overview

```mermaid
graph TB
    subgraph Client Side
        GC[Game Client]
    end
    
    subgraph rAthena Servers
        LS[Login Server:6900]
        CS[Char Server:6121]
        MS[Map Server:5121]
        WS[Web Server:8080]
    end
    
    subgraph AI Bridge
        AB[ro-ai-bridge:3000]
        OLL[Ollama LLM]
        QDR[Qdrant Vector DB]
    end
    
    subgraph Database
        DB[(MariaDB)]
    end
    
    GC -->|1. Auth| LS
    GC -->|2. Char Select| CS
    GC -->|3. Game Play| MS
    GC -->|4. Web API| WS
    
    LS <--> DB
    CS <--> DB
    MS <--> DB
    WS <--> DB
    
    MS -.->|HTTP/Socket| AB
    WS -.->|HTTP| AB
    
    AB --> OLL
    AB --> QDR
    AB <--> DB
```

---

## 1. 🎮 Game Client ↔ rAthena Communication

### 1.1 Packet-Based Protocol

Game Client สื่อสารกับ rAthena ผ่าน **Binary Packets** บน TCP Socket:

| Connection   | Port | Protocol    | Purpose                       |
| ------------ | ---- | ----------- | ----------------------------- |
| Login Server | 6900 | TCP Packets | Authentication, Server List   |
| Char Server  | 6121 | TCP Packets | Character Selection, Creation |
| Map Server   | 5121 | TCP Packets | Gameplay, NPC Interaction     |
| Web Server   | 8080 | HTTP/REST   | Emblem, Config, Party Booking |

### 1.2 NPC Interaction Flow

เมื่อผู้เล่นคุยกับ NPC จะเกิด Packet Flow ดังนี้:

```
[Client]                    [Map Server]                [NPC Script]
   |                            |                           |
   |--- NPC_CLICK packet ----->|                           |
   |                           |--- Execute Script -------->|
   |                           |                           |
   |<-- NPC_DIALOG packet -----|<-- mes "Hello" -----------|
   |                           |                           |
   |--- NPC_NEXT packet ------>|                           |
   |                           |--- Continue Script ------->|
   |                           |                           |
   |<-- NPC_DIALOG packet -----|<-- mes "More text" -------|
   |                           |                           |
   |--- NPC_CLOSE packet ----->|                           |
   |                           |--- End Script ------------>|
   |                           |                           |
```

### 1.3 Key Packet Types

```cpp
// From clif.hpp - Client Interface
PACKET_ZC_NPC_CHAT          = 0x00b4,  // NPC พูดกับผู้เล่น
PACKET_CZ_NPC_CHAT          = 0x00b5,  // ผู้เล่นพูดกับ NPC
PACKET_ZC_SCRIPT_DIALOG     = 0x00b6,  // NPC Dialog
PACKET_CZ_NEXT_DIALOG       = 0x00b9,  // กด Next
PACKET_CZ_CLOSE_DIALOG      = 0x00b6,  // ปิด Dialog
PACKET_CZ_INPUT_EDIT        = 0x01d5,  // Input text
PACKET_CZ_MENU_SELECT       = 0x00b8,  // เลือก Menu
```

---

## 2. 🔧 rAthena Extension Methods for AI Integration

### 2.1 Method A: SQL Database Bridge (Recommended - Easiest)

**หลักการ:** ใช้ `query_sql()` command ใน NPC Script เพื่อสื่อสารผ่าน Database

#### Implementation Architecture

```mermaid
sequenceDiagram
    participant P as Player
    participant NPC as NPC Script
    participant DB as MariaDB
    participant AI as AI Bridge
    participant LLM as Ollama LLM
    
    P->>NPC: คุยกับ NPC
    NPC->>DB: INSERT INTO ai_request
    Note over NPC,DB: session_id, player_msg, context
    
    loop Polling every 100ms
        AI->>DB: SELECT * FROM ai_request WHERE status=pending
        DB->>AI: Return pending requests
    end
    
    AI->>LLM: Generate response
    LLM->>AI: AI response JSON
    AI->>DB: UPDATE ai_response SET response=...
    
    loop NPC waits for response
        NPC->>DB: SELECT response FROM ai_response
        DB->>NPC: Return AI response
    end
    
    NPC->>P: แสดงข้อความจาก AI
```

#### Database Schema for AI Communication

```sql
-- Table: ai_npc_request
CREATE TABLE ai_npc_request (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    session_id VARCHAR(64) NOT NULL,
    npc_id INT NOT NULL,
    npc_name VARCHAR(64),
    char_id INT NOT NULL,
    char_name VARCHAR(24),
    message TEXT NOT NULL,
    context JSON,                    -- Player state, HP, Zeny, etc.
    status ENUM('pending', 'processing', 'completed', 'failed') DEFAULT 'pending',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    processed_at TIMESTAMP NULL,
    INDEX idx_status (status),
    INDEX idx_session (session_id)
);

-- Table: ai_npc_response
CREATE TABLE ai_npc_response (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    request_id BIGINT NOT NULL,
    response_text TEXT,              -- NPC พูดอะไร
    action_type VARCHAR(32),         -- HEAL, BUFF, GIVE_ITEM, WARP, NONE
    action_params JSON,              -- Parameters สำหรับ action
    emotion INT,                     -- Emote ID
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (request_id) REFERENCES ai_npc_request(id)
);
```

#### NPC Script Implementation

```c
// npc/custom/ai_npc.txt
prontera,150,150,4	script	AI Oracle	4_F_KAFRA,{
    // 1. สร้าง Session ID
    .@session$ = "AI_" + getcharid(0) + "_" + gettimetick(2);
    
    // 2. รับข้อความจากผู้เล่น
    mes "[Oracle]";
    mes "สวัสดี ข้าคือ Oracle ผู้รู้ทุกสิ่ง";
    mes "มีอะไรอยากถามไหม?";
    input .@player_msg$;
    
    // 3. เตรียม Context (JSON format)
    .@context$ = "{";
    .@context$ = .@context$ + "\"char_name\":\"" + strcharinfo(0) + "\",";
    .@context$ = .@context$ + "\"base_level\":" + BaseLevel + ",";
    .@context$ = .@context$ + "\"hp_percent\":" + (Hp * 100 / MaxHp) + ",";
    .@context$ = .@context$ + "\"zeny\":" + Zeny;
    .@context$ = .@context$ + "}";
    
    // 4. ส่ง Request ไป Database
    .@query$ = "INSERT INTO ai_npc_request (session_id, npc_id, npc_name, char_id, char_name, message, context) VALUES ('" + .@session$ + "', " + .@npc_id + ", 'AI Oracle', " + getcharid(0) + ", '" + strcharinfo(0) + "', '" + escape_sql(.@player_msg$) + "', '" + escape_sql(.@context$) + "')";
    query_sql(.@query$);
    
    // 5. รอ Response (Polling with timeout)
    .@timeout = 5000;  // 5 วินาที
    .@start = gettimetick(2);
    
    while (gettimetick(2) - .@start < .@timeout) {
        // Check for response
        query_sql("SELECT response_text, action_type, action_params, emotion FROM ai_npc_response r JOIN ai_npc_request q ON r.request_id = q.id WHERE q.session_id = '" + .@session$ + "' AND q.status = 'completed'", .@response$, .@action$, .@params$, .@emotion);
        
        if (.@response$ != "") {
            // Got response!
            break;
        }
        
        // Wait 100ms before next check
        sleep2 100;
    }
    
    // 6. แสดงผล
    if (.@response$ == "") {
        // Timeout - Fallback
        mes "[Oracle]";
        mes "ขออภัย ข้ากำลังง่วงนอน... ลองใหม่ทีหลังนะ";
        close;
    }
    
    // 7. แสดง Emotion (ถ้ามี)
    if (.@emotion > 0) {
        emotion .@emotion;
    }
    
    // 8. แสดงข้อความ
    mes "[Oracle]";
    mes .@response$;
    
    // 9. Execute Action (ถ้ามี)
    if (.@action$ == "HEAL") {
        percentheal 100, 100;
        specialeffect2 EF_HEAL2;
    } else if (.@action$ == "BUFF") {
        // Parse params and cast buff
        sc_start SC_BLESSING, 300000, 10;
        sc_start SC_INCREASEAGI, 300000, 10;
    } else if (.@action$ == "GIVE_ITEM") {
        // Parse item_id and amount from params
        // getitem .@item_id, .@amount;
    }
    
    close;
}
```

#### AI Bridge Implementation (Rust)

```rust
// ro-ai-bridge/src/services/npc_bridge.rs
use sqlx::mysql::MySqlPool;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct NpcRequest {
    pub id: i64,
    pub session_id: String,
    pub npc_id: i32,
    pub npc_name: String,
    pub char_id: i32,
    pub char_name: String,
    pub message: String,
    pub context: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct NpcResponse {
    pub request_id: i64,
    pub response_text: String,
    pub action_type: String,
    pub action_params: serde_json::Value,
    pub emotion: Option<i32>,
}

pub async fn poll_and_process_requests(pool: &MySqlPool) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        // 1. Poll pending requests
        let requests = sqlx::query_as!(
            NpcRequest,
            r#"SELECT id, session_id, npc_id, npc_name, char_id, char_name, message, context
               FROM ai_npc_request 
               WHERE status = 'pending'
               ORDER BY created_at ASC
               LIMIT 10"#
        )
        .fetch_all(pool)
        .await?;

        // 2. Process each request
        for req in requests {
            // Mark as processing
            sqlx::query!(
                "UPDATE ai_npc_request SET status = 'processing' WHERE id = ?",
                req.id
            )
            .execute(pool)
            .await?;

            // Generate AI response
            match generate_ai_response(&req).await {
                Ok(response) => {
                    // Insert response
                    sqlx::query!(
                        r#"INSERT INTO ai_npc_response (request_id, response_text, action_type, action_params, emotion)
                           VALUES (?, ?, ?, ?, ?)"#,
                        response.request_id,
                        response.response_text,
                        response.action_type,
                        response.action_params,
                        response.emotion
                    )
                    .execute(pool)
                    .await?;

                    // Mark as completed
                    sqlx::query!(
                        "UPDATE ai_npc_request SET status = 'completed', processed_at = NOW() WHERE id = ?",
                        req.id
                    )
                    .execute(pool)
                    .await?;
                }
                Err(e) => {
                    // Mark as failed
                    sqlx::query!(
                        "UPDATE ai_npc_request SET status = 'failed' WHERE id = ?",
                        req.id
                    )
                    .execute(pool)
                    .await?;
                    tracing::error!("Failed to process request {}: {}", req.id, e);
                }
            }
        }

        // 3. Sleep before next poll
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
}

async fn generate_ai_response(req: &NpcRequest) -> Result<NpcResponse, Box<dyn std::error::Error>> {
    // Call LLM and generate response
    // ... implementation details in Phase 2
    todo!()
}
```

---

### 2.2 Method B: HTTP Plugin via libcurl (Advanced)

**หลักการ:** แก้ไข rAthena Source Code เพื่อเพิ่มคำสั่ง `http_get()` และ `http_post()`

#### Source Code Modification

```cpp
// src/map/script.cpp - Add new script commands

#include <curl/curl.h>

// HTTP Response buffer
struct HttpResponse {
    char *data;
    size_t size;
};

static size_t write_callback(void *contents, size_t size, size_t nmemb, void *userp) {
    size_t realsize = size * nmemb;
    struct HttpResponse *resp = (struct HttpResponse *)userp;
    
    char *ptr = (char *)realloc(resp->data, resp->size + realsize + 1);
    if (!ptr) return 0;
    
    resp->data = ptr;
    memcpy(&(resp->data[resp->size]), contents, realsize);
    resp->size += realsize;
    resp->data[resp->size] = 0;
    
    return realsize;
}

// http_get("http://localhost:3000/api/ai?msg=hello")
BUILDIN_FUNC(http_get) {
    const char *url = script_getstr(st, 2);
    
    CURL *curl = curl_easy_init();
    if (!curl) {
        script_pushstr(st, "ERROR: Failed to init curl");
        return SCRIPT_CMD_SUCCESS;
    }
    
    struct HttpResponse resp = { .data = (char *)malloc(1), .size = 0 };
    
    curl_easy_setopt(curl, CURLOPT_URL, url);
    curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, write_callback);
    curl_easy_setopt(curl, CURLOPT_WRITEDATA, &resp);
    curl_easy_setopt(curl, CURLOPT_TIMEOUT, 5L);  // 5 second timeout
    
    CURLcode res = curl_easy_perform(curl);
    curl_easy_cleanup(curl);
    
    if (res != CURLE_OK) {
        free(resp.data);
        script_pushstr(st, "ERROR: HTTP request failed");
        return SCRIPT_CMD_SUCCESS;
    }
    
    script_pushstr(st, resp.data);
    free(resp.data);
    return SCRIPT_CMD_SUCCESS;
}

// http_post("http://localhost:3000/api/ai", "{\"msg\":\"hello\"}")
BUILDIN_FUNC(http_post) {
    const char *url = script_getstr(st, 2);
    const char *payload = script_getstr(st, 3);
    
    CURL *curl = curl_easy_init();
    if (!curl) {
        script_pushstr(st, "ERROR: Failed to init curl");
        return SCRIPT_CMD_SUCCESS;
    }
    
    struct HttpResponse resp = { .data = (char *)malloc(1), .size = 0 };
    struct curl_slist *headers = NULL;
    
    headers = curl_slist_append(headers, "Content-Type: application/json");
    
    curl_easy_setopt(curl, CURLOPT_URL, url);
    curl_easy_setopt(curl, CURLOPT_POSTFIELDS, payload);
    curl_easy_setopt(curl, CURLOPT_HTTPHEADER, headers);
    curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, write_callback);
    curl_easy_setopt(curl, CURLOPT_WRITEDATA, &resp);
    curl_easy_setopt(curl, CURLOPT_TIMEOUT, 5L);
    
    CURLcode res = curl_easy_perform(curl);
    
    curl_slist_free_all(headers);
    curl_easy_cleanup(curl);
    
    if (res != CURLE_OK) {
        free(resp.data);
        script_pushstr(st, "ERROR: HTTP request failed");
        return SCRIPT_CMD_SUCCESS;
    }
    
    script_pushstr(st, resp.data);
    free(resp.data);
    return SCRIPT_CMD_SUCCESS;
}

// Register commands
BUILDIN_DEF(http_get, "s"),
BUILDIN_DEF(http_post, "ss"),
```

#### NPC Script with HTTP

```c
prontera,150,150,4	script	AI Healer	4_F_KAFRA,{
    mes "[AI Healer]";
    mes "สวัสดี! บอกอะไรให้ช่วยไหม?";
    input .@msg$;
    
    // Build JSON payload
    .@json$ = "{";
    .@json$ = .@json$ + "\"char_name\":\"" + strcharinfo(0) + "\",";
    .@json$ = .@json$ + "\"message\":\"" + escape_sql(.@msg$) + "\",";
    .@json$ = .@json$ + "\"hp\":" + Hp + ",";
    .@json$ = .@json$ + "\"max_hp\":" + MaxHp + ",";
    .@json$ = .@json$ + "\"zeny\":" + Zeny;
    .@json$ = .@json$ + "}";
    
    // Call AI Bridge
    .@response$ = http_post("http://127.0.0.1:3000/api/npc/chat", .@json$);
    
    // Parse JSON response (simple parsing)
    // Expected: {"text":"...", "action":"HEAL"}
    
    if (.@response$ == "ERROR: HTTP request failed") {
        // Fallback to static response
        mes "ขออภัย ระบบมีปัญหา ลองใหม่ทีหลัง";
        close;
    }
    
    // Extract text and action from JSON
    // ... parsing logic ...
    
    mes .@text$;
    
    if (.@action$ == "HEAL") {
        percentheal 100, 100;
        specialeffect2 EF_HEAL2;
    }
    
    close;
}
```

---

### 2.3 Method C: Built-in Web Server Extension

**หลักการ:** rAthena มี Web Server ในตัว (`src/web/`) ที่ใช้ `cpp-httplib` สามารถเพิ่ม API endpoints ได้

#### Adding Custom API Endpoint

```cpp
// src/web/ai_controller.cpp (New file)

#include "http.hpp"
#include "auth.hpp"
#include <common/sql.hpp>
#include <nlohmann/json.hpp>

using json = nlohmann::json;

// POST /ai/npc/chat
HANDLER_FUNC(ai_npc_chat) {
    // 1. Parse request body
    json req_body = json::parse(req.body, nullptr, false);
    if (req_body.is_discarded()) {
        res.status = 400;
        res.set_content("{\"error\":\"Invalid JSON\"}", "application/json");
        return;
    }
    
    // 2. Validate required fields
    if (!req_body.contains("char_id") || !req_body.contains("message")) {
        res.status = 400;
        res.set_content("{\"error\":\"Missing required fields\"}", "application/json");
        return;
    }
    
    int char_id = req_body["char_id"].get<int>();
    std::string message = req_body["message"].get<std::string>();
    
    // 3. Get character info from database
    // ... query char data ...
    
    // 4. Call AI service (internal or external)
    json ai_response = call_ai_service(message, char_id);
    
    // 5. Return response
    res.set_content(ai_response.dump(), "application/json");
}

// Register route in web.cpp
// http_server->Post("/ai/npc/chat", ai_npc_chat);
```

---

### 2.4 Method D: External Service with Direct DB Access (Rust)

**หลักการ:** Rust service อ่านจาก Database ตรงๆ แล้วเขียนกลับ (เหมาะสำหรับ Project Mimir ที่ใช้ Rust stack)

#### Rust AI Bridge Implementation

```rust
// ro-ai-bridge/src/bin/npc_bridge.rs
//! NPC Bridge Service - Polls database for NPC requests and processes them with AI
//! 
//! Usage: cargo run --bin npc_bridge

use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use serde::{Deserialize, Serialize};
use reqwest::Client;
use std::time::Duration;
use tracing::{info, error, warn};
use chrono::{DateTime, Utc};

/// Configuration for the NPC Bridge
#[derive(Debug, Clone)]
pub struct BridgeConfig {
    pub database_url: String,
    pub ollama_url: String,
    pub poll_interval_ms: u64,
    pub max_batch_size: usize,
    pub request_timeout_sec: u64,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "mysql://ragnarok:password@127.0.0.1/ragnarok".to_string()),
            ollama_url: std::env::var("OLLAMA_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:11434".to_string()),
            poll_interval_ms: 100,
            max_batch_size: 10,
            request_timeout_sec: 5,
        }
    }
}

/// NPC Request from database
#[derive(Debug, Deserialize, sqlx::FromRow)]
pub struct NpcRequest {
    pub id: i64,
    pub session_id: String,
    pub npc_id: i32,
    pub npc_name: String,
    pub char_id: i32,
    pub char_name: String,
    pub message: String,
    pub context: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

/// NPC Response to write back
#[derive(Debug, Serialize)]
pub struct NpcResponse {
    pub request_id: i64,
    pub response_text: String,
    pub action_type: String,
    pub action_params: serde_json::Value,
    pub emotion: Option<i32>,
}

/// AI Response from Ollama
#[derive(Debug, Deserialize)]
struct OllamaResponse {
    response: String,
}

/// AI Generated Response Content
#[derive(Debug, Deserialize)]
struct AiContent {
    text: String,
    action: String,
    #[serde(default)]
    params: serde_json::Value,
    #[serde(default)]
    emotion: Option<i32>,
}

/// Main NPC Bridge Service
pub struct NpcBridgeService {
    pool: MySqlPool,
    http_client: Client,
    config: BridgeConfig,
}

impl NpcBridgeService {
    /// Create a new NPC Bridge Service
    pub async fn new(config: BridgeConfig) -> Result<Self, Box<dyn std::error::Error>> {
        // Create database pool
        let pool = MySqlPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(5))
            .connect(&config.database_url)
            .await?;
        
        // Create HTTP client
        let http_client = Client::builder()
            .timeout(Duration::from_secs(config.request_timeout_sec))
            .build()?;
        
        Ok(Self { pool, http_client, config })
    }
    
    /// Main processing loop
    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        info!("NPC Bridge Service started");
        info!("Polling interval: {}ms", self.config.poll_interval_ms);
        
        loop {
            match self.process_batch().await {
                Ok(processed) => {
                    if processed > 0 {
                        info!("Processed {} requests", processed);
                    }
                }
                Err(e) => {
                    error!("Error processing batch: {}", e);
                }
            }
            
            // Sleep before next poll
            tokio::time::sleep(Duration::from_millis(self.config.poll_interval_ms)).await;
        }
    }
    
    /// Process a batch of pending requests
    async fn process_batch(&self) -> Result<usize, Box<dyn std::error::Error>> {
        // Start transaction
        let mut tx = self.pool.begin().await?;
        
        // Fetch pending requests with FOR UPDATE SKIP LOCKED (to prevent race conditions)
        let requests = sqlx::query_as!(
            NpcRequest,
            r#"SELECT id, session_id, npc_id, npc_name, char_id, char_name, message, context, created_at
               FROM ai_npc_request 
               WHERE status = 'pending'
               ORDER BY created_at ASC
               LIMIT ?
               FOR UPDATE SKIP LOCKED"#,
            self.config.max_batch_size as i64
        )
        .fetch_all(&mut *tx)
        .await?;
        
        if requests.is_empty() {
            tx.rollback().await?;
            return Ok(0);
        }
        
        let request_ids: Vec<i64> = requests.iter().map(|r| r.id).collect();
        
        // Mark all as processing
        sqlx::query!(
            r#"UPDATE ai_npc_request 
               SET status = 'processing' 
               WHERE id IN (?)"#,
            request_ids.clone()
        )
        .execute(&mut *tx)
        .await?;
        
        tx.commit().await?;
        
        // Process each request
        let mut processed = 0;
        for req in requests {
            match self.process_single_request(&req).await {
                Ok(response) => {
                    // Insert response
                    if let Err(e) = self.insert_response(&response).await {
                        error!("Failed to insert response for request {}: {}", req.id, e);
                        self.mark_failed(req.id).await?;
                    } else {
                        self.mark_completed(req.id).await?;
                        processed += 1;
                    }
                }
                Err(e) => {
                    error!("Failed to process request {}: {}", req.id, e);
                    self.mark_failed(req.id).await?;
                }
            }
        }
        
        Ok(processed)
    }
    
    /// Process a single NPC request
    async fn process_single_request(&self, req: &NpcRequest) -> Result<NpcResponse, Box<dyn std::error::Error>> {
        // Build prompt for AI
        let prompt = self.build_prompt(req);
        
        // Call Ollama
        let ollama_response = self.call_ollama(&prompt).await?;
        
        // Parse AI response
        let ai_content: AiContent = serde_json::from_str(&ollama_response.response)
            .map_err(|e| format!("Failed to parse AI response: {}", e))?;
        
        // Validate action type
        let action_type = self.validate_action(&ai_content.action);
        
        Ok(NpcResponse {
            request_id: req.id,
            response_text: ai_content.text,
            action_type,
            action_params: ai_content.params,
            emotion: ai_content.emotion,
        })
    }
    
    /// Build prompt for AI
    fn build_prompt(&self, req: &NpcRequest) -> String {
        format!(
            r#"You are "{}", an NPC in a Ragnarok Online game.

Player Context:
- Name: {}
- NPC: {}

Current Context:
{}

Player says: "{}"

Respond as JSON with this exact format:
{{
    "text": "Your response to the player",
    "action": "NONE" | "HEAL" | "BUFF" | "GIVE_ITEM" | "WARP",
    "params": {{}},
    "emotion": null or emotion_id (0-68)
}}

Rules:
- Keep responses short (1-3 sentences)
- Only use actions that make sense for your NPC type
- Be helpful but stay in character
- If player asks for something unreasonable, politely decline"#,
            req.npc_name,
            req.char_name,
            req.npc_name,
            serde_json::to_string_pretty(&req.context).unwrap_or_default(),
            req.message
        )
    }
    
    /// Call Ollama API
    async fn call_ollama(&self, prompt: &str) -> Result<OllamaResponse, Box<dyn std::error::Error>> {
        let url = format!("{}/api/generate", self.config.ollama_url);
        
        let payload = serde_json::json!({
            "model": "gemma:2b",
            "prompt": prompt,
            "stream": false,
            "options": {
                "temperature": 0.7,
                "top_p": 0.9,
                "num_predict": 256
            }
        });
        
        let response = self.http_client
            .post(&url)
            .json(&payload)
            .send()
            .await?
            .error_for_status()?
            .json::<OllamaResponse>()
            .await?;
        
        Ok(response)
    }
    
    /// Validate and sanitize action type
    fn validate_action(&self, action: &str) -> String {
        const ALLOWED_ACTIONS: &[&str] = &["NONE", "HEAL", "BUFF", "GIVE_ITEM", "WARP"];
        
        let action_upper = action.to_uppercase();
        if ALLOWED_ACTIONS.contains(&action_upper.as_str()) {
            action_upper
        } else {
            "NONE".to_string()
        }
    }
    
    /// Insert response into database
    async fn insert_response(&self, response: &NpcResponse) -> Result<(), Box<dyn std::error::Error>> {
        sqlx::query!(
            r#"INSERT INTO ai_npc_response 
               (request_id, response_text, action_type, action_params, emotion)
               VALUES (?, ?, ?, ?, ?)"#,
            response.request_id,
            response.response_text,
            response.action_type,
            response.action_params,
            response.emotion
        )
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    /// Mark request as completed
    async fn mark_completed(&self, request_id: i64) -> Result<(), Box<dyn std::error::Error>> {
        sqlx::query!(
            r#"UPDATE ai_npc_request 
               SET status = 'completed', processed_at = NOW() 
               WHERE id = ?"#,
            request_id
        )
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    /// Mark request as failed
    async fn mark_failed(&self, request_id: i64) -> Result<(), Box<dyn std::error::Error>> {
        sqlx::query!(
            r#"UPDATE ai_npc_request SET status = 'failed' WHERE id = ?"#,
            request_id
        )
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Load configuration
    let config = BridgeConfig::default();
    
    // Create and run service
    let service = NpcBridgeService::new(config).await?;
    service.run().await
}
```

#### Cargo.toml Dependencies

```toml
# ro-ai-bridge/Cargo.toml
[dependencies]
sqlx = { version = "0.7", features = ["mysql", "runtime-tokio", "chrono", "json"] }
reqwest = { version = "0.11", features = ["json"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
tracing-subscriber = "0.3"
chrono = { version = "0.4", features = ["serde"] }
```

#### Running the Service

```bash
# Set environment variables
export DATABASE_URL="mysql://ragnarok:password@127.0.0.1/ragnarok"
export OLLAMA_URL="http://127.0.0.1:11434"

# Run the bridge
cd ro-ai-bridge
cargo run --bin npc_bridge
```

#### Docker Compose Integration

```yaml
# docker-compose.yml - Add this service
services:
  npc-bridge:
    build:
      context: ./ro-ai-bridge
      target: npc-bridge
    environment:
      - DATABASE_URL=mysql://ragnarok:${MARIADB_PASSWORD}@mariadb/ragnarok
      - OLLAMA_URL=http://host.docker.internal:11434
    depends_on:
      - mariadb
    restart: unless-stopped
    networks:
      - mimir_network
```

---

## 3. 📋 Method Comparison

| Method                  | Difficulty | Latency    | Reliability | Maintenance |
| ----------------------- | ---------- | ---------- | ----------- | ----------- |
| **A: SQL Bridge**       | ⭐ Easy     | ~200-500ms | ✅ High      | ✅ Low       |
| **B: HTTP Plugin**      | ⭐⭐⭐ Hard   | ~50-200ms  | ⚠️ Medium    | ⚠️ High      |
| **C: Web Server**       | ⭐⭐ Medium  | ~100-300ms | ✅ High      | ⚠️ Medium    |
| **D: External Service** | ⭐ Easy     | ~200-500ms | ✅ High      | ✅ Low       |

### Recommendation

**สำหรับ Project Mimir แนะนำ Method A (SQL Bridge)** เพราะ:
1. ไม่ต้องแก้ไข rAthena Source Code
2. ใช้ `query_sql()` ที่มีอยู่แล้ว
3. ง่ายต่อ Debug และ Monitor
4. AI Bridge สามารถ Scale แยกได้
5. Fallback ง่าย (ถ้า AI ไม่ตอบ ใช้ static message)

---

## 4. 🚀 Implementation Roadmap

### Phase 1: Database Setup
- [ ] Create `ai_npc_request` and `ai_npc_response` tables
- [ ] Create basic NPC script with `query_sql()` polling
- [ ] Test polling mechanism

### Phase 2: AI Bridge Development
- [ ] Implement Rust polling service
- [ ] Connect to Ollama LLM
- [ ] Implement response generation
- [ ] Add timeout and fallback logic

### Phase 3: Production Hardening
- [ ] Add connection pooling
- [ ] Implement circuit breaker
- [ ] Add monitoring and logging
- [ ] Load testing

---

## 5. 🔒 Security Considerations

### Input Sanitization
```c
// Always use escape_sql() for user input
.@safe_msg$ = escape_sql(.@player_msg$);
query_sql("INSERT INTO ai_npc_request (message) VALUES ('" + .@safe_msg$ + "')");
```

### Rate Limiting
```sql
-- Check rate limit before processing
SELECT COUNT(*) INTO @recent_requests
FROM ai_npc_request
WHERE char_id = ? AND created_at > DATE_SUB(NOW(), INTERVAL 1 MINUTE);

IF @recent_requests > 5 THEN
    -- Reject request
END IF;
```

### Action Whitelist
```rust
// Only allow specific actions
const ALLOWED_ACTIONS: &[&str] = &["HEAL", "BUFF", "GIVE_ITEM", "WARP", "NONE"];

if !ALLOWED_ACTIONS.contains(&response.action_type.as_str()) {
    response.action_type = "NONE";
}
```

---

## 6. 📚 References

- [`rathena/doc/script_commands.txt`](../rathena/doc/script_commands.txt) - Script command reference
- [`rathena/doc/source_doc.txt`](../rathena/doc/source_doc.txt) - Source code documentation
- [`rathena/src/web/web.cpp`](../rathena/src/web/web.cpp) - Built-in web server
- [`docs/AI_NPC_Integration_Design.md`](./AI_NPC_Integration_Design.md) - AI integration design
- [`docs/Implementation_Plan_Project-Mimir.md`](./Implementation_Plan_Project-Mimir.md) - Implementation plan
