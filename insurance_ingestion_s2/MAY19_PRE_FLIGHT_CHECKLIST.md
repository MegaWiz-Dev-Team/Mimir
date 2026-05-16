# May 19 Pre-Flight Checklist
## Infrastructure & Environment Verification

**Purpose:** Ensure everything is ready for Day 1 (May 19 9:00 AM) start  
**Timeline:** May 16-18 (before execution begins)  
**Owner:** You

---

## ✅ Part 1: RefGraph Rust (15 min)

### 1.1 Code is Ready

```bash
cd /Users/mimir/Developer/Mimir/refgraph-rs

# Verify all source files exist
ls -la src/*.rs
# Should see: lib.rs, types.rs, error.rs, manifest.rs, extract.rs, dedup.rs, graph.rs, mimir.rs, main.rs

# Verify Cargo.toml
cat Cargo.toml | grep edition
# Should show: edition = "2021"
```

### 1.2 Builds Cleanly

```bash
# Clean build
cargo clean
cargo build --release

# Expected output:
#   Compiling refgraph v1.0.0
#   Finished release [optimized] target(s) in X.XXs
```

**If it fails:** Check Cargo.toml dependencies, fix any compiler errors before May 19.

### 1.3 Tests Pass

```bash
# Run all tests
cargo test --lib

# Expected output:
#   test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured
```

**If tests fail:** Debug now, don't start May 19 with broken tests.

### 1.4 Documentation Accessible

```bash
# Read the key files you'll need
cat refgraph-rs/README.md | head -50
cat refgraph-rs/TDD_WORKFLOW.md | grep "Day 1" -A 30
cat refgraph-rs/EXAMPLE_TDD_EXTRACT.md | head -100
```

---

## ✅ Part 2: K8s Services (10 min)

### 2.1 Services Running

```bash
# Check all pods are running
kubectl get pods -n asgard | grep -E "bifrost|mimir|neo4j|qdrant|heimdall"

# Expected: all with status "Running" (1/1)
# If any are pending/crashed, troubleshoot before May 19
```

**Troubleshoot if needed:**
```bash
# Check pod status
kubectl describe pod <pod-name> -n asgard

# Check logs
kubectl logs <pod-name> -n asgard --tail=50

# Restart if needed
kubectl rollout restart deployment/<service-name> -n asgard
kubectl rollout status deployment/<service-name> -n asgard
```

### 2.2 Port Forwarding Active

```bash
# Start port forwarding (run in background)
kubectl port-forward -n asgard svc/mimir 8000:8000 &
kubectl port-forward -n asgard svc/neo4j 7687:7687 &
kubectl port-forward -n asgard svc/qdrant 6333:6333 &
kubectl port-forward -n asgard svc/heimdall 8001:8001 &

# Verify all are accessible
curl -s http://localhost:8000/health && echo "✅ Mimir"
curl -s http://localhost:6333/health && echo "✅ Qdrant"
```

### 2.3 Neo4j Ready

```bash
# Test connection
neo4j-cli db cypher --database neo4j "RETURN 1 as result;"

# Or via bolt (if installed):
# bolt localhost:7687 -u neo4j -p <password>

# Expected: successful connection, no auth errors
```

**If Neo4j fails:**
```bash
# Check Neo4j pod
kubectl logs neo4j-xxxxx -n asgard | grep -i error

# Restart
kubectl rollout restart deployment/neo4j -n asgard
```

---

## ✅ Part 3: Environment Variables (5 min)

### 3.1 Set Up .env (if needed)

```bash
cd /Users/mimir/Developer/Mimir/refgraph-rs

# Create .env if doesn't exist
cat > .env << 'EOF'
RUST_LOG=info
NEO4J_URI=bolt://localhost:7687
NEO4J_USER=neo4j
NEO4J_PASSWORD=<password>
MIMIR_URI=http://localhost:8000
MIMIR_API_KEY=<if-required>
EOF

# Verify it loads (optional)
source .env
echo $NEO4J_URI  # Should print: bolt://localhost:7687
```

### 3.2 Verify Connectivity

```bash
# Test Neo4j from Rust
cargo run --release --bin refgraph --example neo4j_test

# Or a quick test:
./target/release/refgraph --test
# Expected output:
#   ✅ Test 1: RefGraph creation
#   ✅ Test 2: Empty consolidation
#   ✅ Test 3: Sample consolidation
#   ✅ All tests passed!
```

---

## ✅ Part 4: Git Ready (5 min)

### 4.1 Repository State

```bash
cd /Users/mimir/Developer/Mimir/refgraph-rs

# Check status
git status
# Expected: clean working directory ("nothing to commit")

# Check recent commits
git log --oneline | head -5
# Should see your previous commits with TDD/extract/etc.
```

### 4.2 Branch Ready

```bash
# Current branch
git branch -v
# Should be on main or feature branch, up to date

# Optional: create release branch
git checkout -b release/v1.0.0
git push origin release/v1.0.0
```

---

## ✅ Part 5: Daily Workflow Setup (5 min)

### 5.1 Create Daily Log

```bash
# Create a simple log file to track progress
cat > /Users/mimir/Developer/Mimir/insurance_ingestion_s2/S1_DAILY_LOG.md << 'EOF'
# S1 Daily Log (May 19-28)

## May 19 (Day 1)
- [ ] Read extract.rs
- [ ] Copy 15 tests
- [ ] Implement tests → all green
- [ ] Tests: 15/15 passing
- [ ] Commit: "feat: entity extraction with TDD"

## May 20 (Day 2)
- [ ] (TBD)

...
EOF

echo "✅ Daily log created"
```

### 5.2 Set Up Terminal Workspace

```bash
# Create an organized terminal layout (optional)
# Terminal 1: Main work
cd /Users/mimir/Developer/Mimir/refgraph-rs

# Terminal 2: Watch tests (optional)
cd /Users/mimir/Developer/Mimir/refgraph-rs
cargo watch -x test

# Terminal 3: Port forwarding
kubectl port-forward -n asgard svc/mimir 8000:8000 &
kubectl port-forward -n asgard svc/neo4j 7687:7687 &
```

---

## ✅ Part 6: Documentation Review (10 min)

### 6.1 Read These Before May 19

```bash
# 1. SOLO_EXECUTION_PLAN.md (you're here!)
cat insurance_ingestion_s2/SOLO_EXECUTION_PLAN.md | head -100

# 2. TDD Workflow (understand the cycle)
cat refgraph-rs/TDD_WORKFLOW.md | grep -A 50 "## TDD Workflow (Cycle)"

# 3. Day 1 example tests (what you'll implement)
cat refgraph-rs/EXAMPLE_TDD_EXTRACT.md | head -150
```

### 6.2 Quick Reference Bookmarks

Save these locations for quick access:

```
refgraph-rs/src/extract.rs           ← May 19 target (207 lines)
refgraph-rs/src/dedup.rs             ← May 20 target (198 lines)
refgraph-rs/src/graph.rs             ← May 21 target (269 lines)
refgraph-rs/EXAMPLE_TDD_EXTRACT.md   ← Copy tests from here
refgraph-rs/TDD_WORKFLOW.md          ← Reference any time
insurance_ingestion_s2/SOLO_EXECUTION_PLAN.md  ← Your master plan
insurance_ingestion_s2/S1_DAILY_LOG.md         ← Track progress
```

---

## ✅ Final Verification Checklist

Run this on May 19 morning before 9:00 AM:

```bash
# 1. Code compiles
cd /Users/mimir/Developer/Mimir/refgraph-rs
cargo build --release 2>&1 | tail -5
# Should end with: Finished release [optimized]

# 2. Tests exist and pass
cargo test --lib 2>&1 | tail -3
# Should end with: test result: ok.

# 3. Services are up
curl -s http://localhost:8000/health > /dev/null && echo "✅ Mimir"
curl -s http://localhost:6333/health > /dev/null && echo "✅ Qdrant"

# 4. Git is ready
git status | grep "nothing to commit" > /dev/null && echo "✅ Git clean"

# 5. You can read the test file
cat /Users/mimir/Developer/Mimir/refgraph-rs/EXAMPLE_TDD_EXTRACT.md | wc -l
# Should show: 500+ lines
```

---

## Troubleshooting

### RefGraph Won't Compile

```bash
# Check Rust version
rustc --version
# Should be: 1.75+ (check cargo update)

# Check all dependencies
cargo check

# Look for specific error
cargo build 2>&1 | grep "error\["
```

**Fix:** Update Cargo.toml, run `cargo update`, try again.

### Services Won't Start

```bash
# Check pod status
kubectl describe pod <pod> -n asgard | grep -i "status\|error"

# Check logs
kubectl logs <pod> -n asgard | tail -20

# Force restart
kubectl delete pod <pod> -n asgard
# Pod will automatically restart
```

### Port Forwarding Fails

```bash
# Check if port is already in use
lsof -i :8000

# Kill existing process
kill -9 <PID>

# Restart port forward
kubectl port-forward -n asgard svc/mimir 8000:8000
```

### Neo4j Connection Error

```bash
# Check Neo4j is running
kubectl get pod -n asgard | grep neo4j | grep Running

# Check credentials in .env
cat .env | grep NEO4J

# Try test connection
neo4j-cli cypher "RETURN 1;"

# If failing: check Neo4j pod logs
kubectl logs neo4j-xxxxx -n asgard | tail -20
```

---

## Ready for May 19?

After completing all checkboxes above, you're ready. On May 19 at 9:00 AM:

```bash
# 1. Make sure you're in the right directory
cd /Users/mimir/Developer/Mimir/refgraph-rs

# 2. Open extract.rs
code src/extract.rs  # or your editor

# 3. Read EXAMPLE_TDD_EXTRACT.md side-by-side
code ../insurance_ingestion_s2/EXAMPLE_TDD_EXTRACT.md

# 4. Start copying tests
# (See Day 1 in SOLO_EXECUTION_PLAN.md)

# 5. Run: cargo test --lib extract::tests
# (Watch them all FAIL - this is correct!)

# 6. Implement to make them pass
# Done for the day when: All 15 tests passing ✅
```

---

## Daily Standup Template (5 min each morning)

```
Yesterday:
  ✅ Tests passing: 15/15
  ✅ Code coverage: 85%
  ☐ Blockers: None

Today:
  ☐ Target: dedup.rs refinement
  ☐ Tests to write: 8
  ☐ Success = <100ms for 1000 entities

Tomorrow:
  ☐ Depends on: Dedup tests passing
  ☐ Fallback if stuck: Defer optimization to Day 7
```

---

**Status**: Pre-flight checklist ready  
**Next**: Run through checklist May 16-18  
**Go Live**: May 19, 9:00 AM  

🚀 You've got this!
