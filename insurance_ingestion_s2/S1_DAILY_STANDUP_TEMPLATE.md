# S1 Sprint: Daily Standup Template + Metrics
## Use This Format Every Day (9:00 AM)

---

## 📝 Daily Standup Format

**Slack Post (copy-paste each morning):**

```
🎯 S1 STANDUP — DAY [N] | [Date]
══════════════════════════════════════════════════════

✅ COMPLETED YESTERDAY:
  • [Task 1] — [Owner: name]
  • [Task 2] — [Owner: name]
  • [Task 3] — [Owner: name]

🔄 IN PROGRESS TODAY:
  • [Task 1] — [Owner: name] — ETA: [time]
  • [Task 2] — [Owner: name] — ETA: [time]

🚧 BLOCKERS:
  • [Blocker 1] — [Impact: HIGH/MEDIUM/LOW] — [Escalated to: name]
  • [Blocker 2] — [Impact: HIGH/MEDIUM/LOW] — [Status: RESOLVED/IN-PROGRESS/PENDING]

📊 METRICS:
  • Chunks extracted today: [N]
  • Chunks validated: [N] (PII: [0] errors)
  • Chunks ingested to Mimir: [N]
  • Test queries run: [N] (Hit Rate: [%])
  • Git commits: [N]
  • Code review: [pending/complete]

🎯 PHASE PROGRESS:
  S1.1 (Extract): [████████░░] 80%
  S1.2 (Chunk):   [██████░░░░] 60%
  S1.3 (Entity):  [████░░░░░░] 40%
  S1.4 (Ingest):  [██░░░░░░░░] 20%
  S1.5 (Validate):[░░░░░░░░░░] 0%

📈 RUNNING TOTAL:
  Chunks extracted: 450/950 (47%)
  Entities found: 245/500+ (49%)
  Neo4j relationships: 780/1000+ (78%)
  Hit Rate@3 baseline: 71% (target: ≥75% by May 22)

⏰ CAPACITY:
  Data Eng: [1.5 FTE] at [%] capacity
  Backend: [1.5 FTE] at [%] capacity
  QA: [0.75 FTE] at [%] capacity
  Tech Lead: [0.25 FTE] at [%] capacity

🎬 ACTION ITEMS FOR TODAY:
  • [ ] Item 1 — Assigned to [name]
  • [ ] Item 2 — Assigned to [name]
  • [ ] Item 3 — Assigned to [name]

📍 NEXT CHECKPOINT:
  [Tomorrow's plan in 1 sentence]
══════════════════════════════════════════════════════
```

---

## 📊 Metrics Tracking Spreadsheet

**Create a Google Sheet with these columns:**

```
Date    | Phase | Task              | Owner      | Status    | Chunks | Entities | Hit Rate | Blockers
--------|-------|-------------------|------------|-----------|--------|----------|----------|----------
May 18  | S1.1  | Extract URL 1     | Data Eng   | Complete  | 45     | 12       | N/A      | None
May 18  | S1.1  | Extract URL 2     | Data Eng   | Complete  | 52     | 15       | N/A      | Rate limited
May 18  | S1.2  | Validate chunks   | QA        | In prog   | 97     | 27       | N/A      | Waiting on extract
May 19  | S1.2  | Chunk validation  | QA        | Complete  | 97     | 27       | 68%      | None
May 19  | S1.3  | Entity extraction | Backend   | In prog   | 97     | 45       | 68%      | NER model training
May 20  | S1.3  | Neo4j ingestion   | Backend   | In prog   | 97     | 89       | 71%      | None
...     | ...   | ...               | ...       | ...       | ...    | ...      | ...      | ...
May 22  | S1.4  | Test queries (GATE) | QA      | PENDING   | 450    | 245      | TBD      | DECISION POINT
May 27  | S1.5  | Final validation  | All       | PENDING   | 950    | 500+     | ≥75%     | GO/NO-GO
```

**Formulas:**
```
% Complete = Chunks today / 950 * 100
% Entities = Entities today / 500 * 100
Days remaining = 27 - current_day
Avg chunks/day = Total chunks / Days elapsed
Velocity: Can we hit 950 by May 27?
```

---

## 📈 Real-Time Metrics Dashboard

**Create daily automated report:**

```bash
#!/bin/bash
# save as: scripts/daily_metrics.sh
# run at: 5:00 PM daily

echo "🎯 S1 SPRINT METRICS — $(date +%Y-%m-%d)"
echo "═════════════════════════════════════════════"

# Count chunks
TOTAL_CHUNKS=$(wc -l data/output/phase1_chunks.jsonl 2>/dev/null | awk '{print $1}')
echo "📊 Chunks extracted: $TOTAL_CHUNKS / 950"

# Count entities in Neo4j
TOTAL_ENTITIES=$(cypher-shell -u neo4j -p $NEO4J_PASSWORD "MATCH (e:Entity) RETURN COUNT(e);" 2>/dev/null)
echo "📊 Entities in Neo4j: $TOTAL_ENTITIES / 500+"

# Count relationships
TOTAL_RELS=$(cypher-shell -u neo4j -p $NEO4J_PASSWORD "MATCH ()-[r]->() RETURN COUNT(r);" 2>/dev/null)
echo "📊 Relationships: $TOTAL_RELS / 1000+"

# Check PII issues
PII_ISSUES=$(grep -c '"pii_detected": true' data/output/phase1_chunks.jsonl 2>/dev/null || echo "0")
echo "🔒 PII issues: $PII_ISSUES (target: 0)"

# Test Hit Rate (if test queries run)
if [ -f data/output/test_results.jsonl ]; then
  HIT_RATE=$(jq -r '.hit_rate_at_3' data/output/test_results.jsonl | head -1)
  echo "🎯 Hit Rate@3: $HIT_RATE% (target: ≥75% by May 22)"
fi

# Days elapsed & velocity
DAYS_ELAPSED=$(( ($(date +%s) - $(date -d "May 18" +%s)) / 86400 ))
CHUNKS_PER_DAY=$(( TOTAL_CHUNKS / DAYS_ELAPSED ))
echo "⚡ Velocity: $CHUNKS_PER_DAY chunks/day"
echo "📅 Days elapsed: $DAYS_ELAPSED / 10"

echo "═════════════════════════════════════════════"
```

**Run daily at 5:00 PM:**
```bash
# Add to crontab:
0 17 * * * cd /Users/mimir/Developer/Mimir/insurance_ingestion_s2 && bash scripts/daily_metrics.sh >> logs/metrics.log
```

---

## 🚨 Escalation Decision Tree

**When blocker occurs:**

```
BLOCKER OCCURS
    ↓
Tech Lead diagnoses (⏱️ 10 min max)
    ↓
Is it fixable by team within 1 hour?
    ├─ YES → Fix it, log it, continue
    │   └─ Update BLOCKER column in spreadsheet
    │
    └─ NO → Check severity
        ├─ HIGH (blocks multiple tasks)
        │   ├─ Infrastructure issue (K8s, DB)?
        │   │   └─ Escalate to DevOps immediately (SLACK)
        │   │
        │   └─ Script/code issue?
        │       └─ Tech Lead troubleshoot (30 min)
        │       └─ If not fixed → Escalate to mentor
        │
        └─ MEDIUM (blocks 1 task)
            └─ Work on different task
            └─ Come back to blocker next day
            └─ Log in spreadsheet
        
        └─ LOW (minor issue)
            └─ Continue work
            └─ Fix if time permits

BLOCKER RESOLUTION
    ↓
Update standup with:
  • What was the issue?
  • How was it fixed?
  • How long did it take?
  └─ Share learnings with team
```

---

## 📋 Daily Checklist (For Tech Lead)

**Every morning before standup:**

```
☐ Check Slack #insurance-s1-sprint for overnight issues
☐ Review yesterday's metrics (chunks, entities, hit rate)
☐ Check git commits (everyone committed yesterday?)
☐ Verify all scripts running without errors
☐ Check K8s pod status (no restarts overnight?)
☐ Review blockers list (any new ones? Any resolved?)
☐ Update tracking spreadsheet with yesterday's numbers
☐ Prepare standup talking points (20 sec per person)
☐ Send standup reminder 5 min before meeting
☐ Run metrics.sh and post results to Slack
```

---

## 💬 Slack Channel: #insurance-s1-sprint

**Daily Posts (automated):**

```
8:55 AM: "🎯 Standup in 5 min (9:00 AM), @channel"

9:30 AM: [Post daily standup template]

5:00 PM: [Post daily metrics report]

Daily: [Automated Hit Rate tracker if test queries running]
```

**How to Integrate with Slack:**

1. **Create Slack webhook:**
   ```
   https://hooks.slack.com/services/YOUR/WEBHOOK/URL
   ```

2. **Script to post metrics:**
   ```bash
   #!/bin/bash
   # scripts/post_slack_metrics.sh
   
   MESSAGE="🎯 S1 STANDUP\n✅ Chunks: $(wc -l data/output/phase1_chunks.jsonl | awk '{print $1}')/950\n📊 Hit Rate: $(grep hit_rate data/output/test_results.jsonl | head -1)"
   
   curl -X POST -H 'Content-type: application/json' \
     --data "{\"text\":\"$MESSAGE\"}" \
     $SLACK_WEBHOOK_URL
   ```

3. **Schedule in crontab:**
   ```bash
   55 8 * * Mon-Fri bash scripts/post_slack_metrics.sh  # 8:55 AM reminder
   0 17 * * Mon-Fri bash scripts/daily_metrics.sh | post_slack_metrics.sh  # 5:00 PM report
   ```

---

## 📅 Weekly Review (Fridays)

**Every Friday 4:00 PM (15 min):**

```
WEEK REVIEW AGENDA:

1. Metrics review (5 min)
   - How many chunks extracted?
   - How many entities?
   - Hit Rate progress?
   - On track to 950 by May 27?

2. Blockers review (5 min)
   - What got resolved?
   - What's pending?
   - Any patterns?

3. Next week plan (5 min)
   - What's the focus?
   - Any risks?
   - Any team adjustments needed?

OUTPUT:
- Update project board
- Send weekly report to manager
- Celebrate wins
```

---

## 🎯 Critical Metrics to Track

**Track these EVERY DAY:**

| Metric | Target | Current | Status | Owner |
|--------|--------|---------|--------|-------|
| Chunks extracted | 950 | [daily] | [OK/AT-RISK] | Data Eng |
| Entities found | 500+ | [daily] | [OK/AT-RISK] | Backend |
| Neo4j relationships | 1000+ | [daily] | [OK/AT-RISK] | Backend |
| PII issues | 0 | [daily] | [OK/AT-RISK] | QA |
| Hit Rate@3 | ≥75% | [May 22] | [PENDING] | QA |
| Code coverage | ≥80% | [daily] | [OK/AT-RISK] | Backend |

**At-Risk Triggers:**
```
If Chunks < 95 per day → ALERT (falling behind)
If Hit Rate < 70% on May 22 → ESCALATE (Plan B activation)
If PII issues > 0 → BLOCK (investigate immediately)
If K8s pods restarting → INVESTIGATE (memory leak?)
```

---

## 📞 On-Call Schedule (During Sprint)

**Who to call if issues:**

```
Data Extraction issues: Data Eng lead (primary), Tech Lead (backup)
Backend/Mimir issues: Backend lead (primary), Tech Lead (backup)
Infrastructure/K8s issues: DevOps on-call (primary)
PII/Security issues: QA lead (primary), Tech Lead (backup)

After hours: Tech Lead on standby (escalations only)
```

---

**Print this page and post in team room** 📌

**Status:** ✅ Ready to track daily  
**Last updated:** May 16, 2026

