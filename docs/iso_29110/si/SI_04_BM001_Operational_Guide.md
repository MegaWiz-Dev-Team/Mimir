# Heimdall Auto-Pipeline — Operational Guide (Qwen3.5-27B)

## Prerequisites

| Service | Command | Port |
|---------|---------|------|
| MariaDB | `brew services start mariadb` | 3306 |
| Qdrant | `docker start qdrant` | 6333 |
| Dashboard | `cd ro-ai-dashboard && npm run dev` | 3001 |

---

## STEP 1: Start Heimdall (Qwen3.5-27B)

```bash
cd ~/Developer/Heimdall
LLM_MODEL="mlx-community/Qwen3.5-27B-4bit" ./scripts/start.sh
```

✅ ต้องเห็น `Heimdall started!` — Backend, Embedding, Gateway ครบ 3 ตัว

**Verify:**
```bash
curl http://localhost:8080/health   # status: "healthy"
```

---

## STEP 2: Start Bridge (ro-ai-bridge)

```bash
cd ~/Developer/Mimir/ro-ai-bridge

HEIMDALL_API_URL=http://localhost:8080/v1 \
HEIMDALL_API_KEY=hml-REDACTED \
EMBEDDING_API_URL=http://localhost:8001/v1 \
cargo run --bin ro-ai-bridge
```

✅ ต้องเห็น `🚀 listening on 0.0.0.0:3000`

> ⚠️ **สำคัญ**: ต้องมีทั้ง 3 env vars:
> - `HEIMDALL_API_URL` → gateway (LLM chat)
> - `HEIMDALL_API_KEY` → API key จาก Heimdall `.env`
> - `EMBEDDING_API_URL` → embedding server ตรง (bypass gateway auth)

---

## STEP 3: Login & Get Token

```bash
TOKEN=$(curl -s http://localhost:3000/api/v1/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"megacare","password":"admin123"}' \
  | python3 -c 'import json,sys; print(json.load(sys.stdin).get("token",""))')

echo $TOKEN   # ตรวจว่าได้ token มา
```

---

## STEP 4: Trigger Auto-Pipeline

### Sources ที่ต้อง run

| Source ID | Name | Chunks | Est. Time (27B) |
|-----------|------|--------|----------------|
| 10 | Sleep Disorders & Deprivation — Clinical Guide | 48 | ~160 min |
| 11 | ENT Clinical Practice Guidelines | 43 | ~140 min |
| 12 | Neurology & Brain Disorders — Sleep Reference | 36 | ~120 min |
| 13 | Sleep & ENT Drug Reference | 34 | ✅ เสร็จแล้ว |
| 14 | CPAP AirSense 10 — คู่มือผู้ใช้ภาษาไทย | 97 | ~320 min |

### Command — เปลี่ยน `SOURCE_ID` ตามต้องการ

```bash
SOURCE_ID=10   # <-- เปลี่ยนตรงนี้

curl -s -X POST "http://localhost:3000/api/v1/sources/${SOURCE_ID}/auto-pipeline" \
  -H "Authorization: Bearer $TOKEN" \
  -H "X-Tenant-Id: 127d37ee-2de2-4094-8993-f7cff046c0ec" \
  -H "Content-Type: application/json" \
  -d '{
    "provider": "heimdall",
    "model": "mlx-community/Qwen3.5-27B-4bit",
    "run_label": "production-qwen27b",
    "skip_completed": false
  }' | python3 -m json.tool
```

✅ ต้องเห็น `"status": "running"`

> Pipeline วิ่ง background — ปิด terminal ไม่ได้ (bridge process ต้องรันอยู่)

---

## STEP 5: Monitor Progress

### วิธี 1: เช็คผ่าน API

```bash
curl -s "http://localhost:3000/api/v1/sources/${SOURCE_ID}/pipeline-status" \
  -H "Authorization: Bearer $TOKEN" \
  -H "X-Tenant-Id: 127d37ee-2de2-4094-8993-f7cff046c0ec" \
  | python3 -m json.tool
```

### วิธี 2: เช็คจาก DB โดยตรง

```bash
python3 -c "
import pymysql
conn = pymysql.connect(host='localhost', user='mimir', password='REDACTED-PW', database='mimir', port=3306)
cur = conn.cursor()
cur.execute('''
  SELECT pr.source_id, pr.status, pr.run_label, pr.model,
    (SELECT COUNT(*) FROM kg_entities WHERE run_label = pr.run_label) as entities,
    pr.started_at, pr.finished_at
  FROM pipeline_runs pr
  WHERE pr.run_label LIKE \"production%\"
  ORDER BY pr.started_at DESC
''')
for r in cur.fetchall():
    print(f'Source {r[0]}: {r[1]} | {r[4]} entities | {r[2]} | {r[5]}')
conn.close()
"
```

### วิธี 3: ดู Bridge log

```bash
tail -f /tmp/bridge.log | grep -E "Step|pipeline|finished"
```

---

## STEP 6: Run ที่เหลือต่อ

เมื่อ source หนึ่งเสร็จ ให้เปลี่ยน `SOURCE_ID` แล้วรัน STEP 4 ซ้ำ:

```bash
# ลำดับแนะนำ (เล็ก → ใหญ่)
SOURCE_ID=12 && curl -s -X POST ...   # 36 chunks, ~2 hr
SOURCE_ID=11 && curl -s -X POST ...   # 43 chunks, ~2.5 hr
SOURCE_ID=10 && curl -s -X POST ...   # 48 chunks, ~2.5 hr
SOURCE_ID=14 && curl -s -X POST ...   # 97 chunks, ~5 hr
```

> ⚠️ Run ทีละ source เท่านั้น — Heimdall serve ได้ทีละ request

---

## STEP 7: Stop Services

```bash
# Stop Heimdall
cd ~/Developer/Heimdall && ./scripts/stop.sh

# Stop Bridge
Ctrl+C (หรือ kill $(lsof -i :3000 -t))
```

---

## Troubleshooting

| Problem | Solution |
|---------|----------|
| Gateway "unhealthy" | `./scripts/stop.sh && ./scripts/start.sh` |
| "No chunks found" | Source ยังไม่ได้ sync/upload — ต้อง upload data ก่อน |
| Embedding 401 | ตรวจว่าตั้ง `EMBEDDING_API_URL=http://localhost:8001/v1` |
| LLM ไม่ตอบ | ตรวจ `HEIMDALL_API_KEY` ว่าตรงกับ `Heimdall/.env` |
| Token หมดอายุ | Run STEP 3 ใหม่ |
