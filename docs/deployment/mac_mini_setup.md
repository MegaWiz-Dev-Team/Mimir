# 🚀 Mac Mini Deployment Guide — Project Mimir

**Target:** macOS (Apple Silicon / Intel Mac Mini)
**Architecture:** Docker services + Rust backend + Next.js frontend

---

## Prerequisites

Install on the Mac Mini:

```bash
# Homebrew
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# Core tools
brew install git rust node docker colima

# Start Docker runtime (no Docker Desktop needed)
colima start --cpu 4 --memory 8 --disk 60
```

> **Colima** เป็น lightweight Docker runtime สำหรับ macOS — ไม่ต้อง Docker Desktop license

---

## 1. Clone & Setup

```bash
# Clone repository
git clone https://github.com/megacare-dev/Project-Mimir.git
cd Project-Mimir

# Copy environment template
cp .env.example .env   # แก้ค่าตาม section ถัดไป
```

## 2. Environment Variables (`.env`)

สร้าง `.env` ที่ root ของ project:

```env
# ═══ Server ═══
PORT=3001

# ═══ MariaDB ═══
DATABASE_URL=mysql://mimir:mimir_password@localhost:3306/mimir
MARIADB_URL=mysql://mimir:mimir_password@localhost:3306/mimir

# ═══ Services ═══
QDRANT_URL=http://localhost:6333
REDIS_URL=redis://localhost:6379
S3_ENDPOINT=http://localhost:9000
S3_BUCKET=mimir-uploads
S3_ACCESS_KEY=minioadmin
S3_SECRET_KEY=minioadmin
S3_REGION=us-east-1

# ═══ LLM — Local (Ollama) ═══
OLLAMA_URL=http://localhost:11434
LOCAL_MODEL=llama3.2
EMBED_MODEL=nomic-embed-text

# ═══ LLM — Gemini (optional) ═══
GEMINI_BASE_URL=https://generativelanguage.googleapis.com
GEMINI_API_KEY=            # ใส่ API key ถ้ามี
GEMINI_MODEL=gemini-2.0-flash

# ═══ LLM — Heimdall (optional) ═══
HEIMDALL_API_URL=          # e.g. http://192.168.x.x:3000
HEIMDALL_API_KEY=          # ใส่ถ้าใช้ Heimdall
HEIMDALL_MODEL=llama3

# ═══ Auth ═══
JWT_SECRET=change-me-to-a-random-string-at-least-32-chars

# ═══ Vault (optional — auto-configured by Docker) ═══
VAULT_ADDR=http://localhost:8200
VAULT_TOKEN=               # จะได้จาก Vault init (ดู Step 3)
VAULT_MOUNT=secret
VAULT_PATH=mimir/secrets

# ═══ Cron ═══
CRON_TICK_SECONDS=60
```

## 3. Start Docker Services

```bash
# Start all infrastructure services
docker compose up -d

# Verify all containers are running
docker compose ps
```

**Expected containers:**

| Container       | Port      | Purpose                    |
| --------------- | --------- | -------------------------- |
| `mimir_mariadb` | 3306      | Primary database           |
| `mimir_qdrant`  | 6333/6334 | Vector database            |
| `mimir_redis`   | 6379      | Cache & session            |
| `mimir_rustfs`  | 9000/9001 | S3-compatible file storage |
| `mimir_vault`   | 8200      | Secrets management         |
| `mimir_neo4j`   | 7474/7687 | Knowledge graph            |

### Vault Token

On first run, Vault auto-initializes and prints the root token:
```bash
docker logs mimir_vault 2>&1 | grep "Root Token"
# 🔑 Root Token: hvs.xxxxx
```

Copy this token to `.env` → `VAULT_TOKEN=hvs.xxxxx`

## 4. Build & Run Backend

```bash
cd ro-ai-bridge

# Run database migrations
cargo install sqlx-cli --no-default-features --features mysql
sqlx migrate run --source mimir-core-ai/migrations

# Build release (optimized)
cargo build --release

# Run
../target/release/ro-ai-bridge
```

> ⚡ Release build ใช้เวลา compile ~3-5 นาทีบน M-series Mac Mini แต่ runtime จะเร็วกว่า debug 10x

## 5. Build & Run Frontend

```bash
cd ro-ai-dashboard

# Install dependencies
npm install

# Build production
npm run build

# Run production server
npm start
```

Frontend จะ serve ที่ `http://localhost:3000`

## 6. (Optional) Install Ollama for Local LLM

```bash
# Install Ollama
brew install ollama

# Pull models
ollama pull llama3.2
ollama pull nomic-embed-text

# Ollama runs automatically at localhost:11434
```

## 7. Create RustFS Bucket

```bash
# Install mc (RustFS/MinIO client)
brew install minio/stable/mc

# Configure
mc alias set mimir http://localhost:9000 minioadmin minioadmin

# Create bucket
mc mb mimir/mimir-uploads
```

## 8. Verify Installation

```bash
# Test backend API
curl http://localhost:3001/api/health

# Test frontend
open http://localhost:3000

# Login with default admin (created during migration)
# Username: admin
# Password: admin123
```

---

## Maintenance Commands

```bash
# View logs
docker compose logs -f --tail=50

# Restart all services
docker compose restart

# Stop everything
docker compose down

# Backup MariaDB
docker exec mimir_mariadb mariadb-dump -u root -proot mimir > backup_$(date +%Y%m%d).sql

# Clear Rust build cache (saves ~8 GB)
cd ro-ai-bridge && cargo clean
```

---

## Troubleshooting

| Symptom                    | Fix                                                                |
| -------------------------- | ------------------------------------------------------------------ |
| `DATABASE_URL must be set` | Check `.env` file exists and `source .env` before running          |
| Vault sealed after restart | Auto-unseals via `entrypoint.sh` — check `docker logs mimir_vault` |
| Qdrant connection refused  | `docker compose up -d qdrant`                                      |
| Neo4j login fails          | Default: `neo4j` / `mimir_neo4j_password`                          |
| RustFS bucket not found    | Run `mc mb mimir/mimir-uploads`                                    |
| Port conflicts             | Check `lsof -i :3001` and stop conflicting services                |
