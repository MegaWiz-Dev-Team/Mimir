# Sprint 2 Insurance Ingestion — Multi-Insurer + Thai + File Upload + OCR

**Sprint:** S2 (June 2026)  
**Objective:** Expand insurance ingestion to multiple insurers + Thai language + file uploads + OCR  
**DRIs:** Data Engineer (Phase 1), Backend (2-4), QA (5), DevOps (infra)  
**Reuse:** 65% from S1, 35% new features

---

## 🆕 What's New in S2

### Feature 1: Multi-Insurer Ingestion
- **Single tenant, multiple insurers:** Add `insurer_id` field to chunks
- **Deduplication:** Prevent duplicate products across insurers
- **Cross-insurer comparison:** Query across all insurers in one search

### Feature 2: Thai Language Support
- **Thai NER:** Extract entities in Thai (medical conditions, product names)
- **Bilingual support:** Run extraction in English, Thai, or both
- **Thai tokenization:** Use pythainlp for proper word boundaries

### Feature 3: File Upload + OCR
- **Multiple file formats:** PDF, DOCX, TXT (text), JPG/PNG (images)
- **Batch upload:** Process multiple files in one Phase 1 run
- **OCR for images:** Via Syn service or fallback to pytesseract
- **Insurer deduplication:** Detect duplicate products from multiple sources

### Feature 4: Document QA Layer (Phase 5 variant)
- **Question-answering:** Answer specific questions about policies
- **Synthetic QA pairs:** Generate training data from chunks
- **Hit Rate metric for QA:** Different from search Hit Rate

### Feature 5: Compliance Checks (Phase 6 new)
- **Regulatory validation:** Check coverage limits against OIC rules
- **Exclusion consistency:** Detect conflicting policy exclusions
- **Audit trail:** Log all changes and decisions

---

## 🚀 Day 1 Setup (June 1, 2026)

### Step 1: Copy S1 & Verify (2 min)

```bash
cd /Users/mimir/Developer/Mimir
git checkout feature/insurance-s2-expansion  # or create from s1 branch
cd insurance_ingestion_s2

# Create virtual environment
python3 -m venv venv
source venv/bin/activate

# Install S2 dependencies (includes Thai NLP, PDF, OCR)
pip install -r requirements.txt
```

### Step 2: Configure Multi-Insurer Setup (5 min)

```bash
# Copy environment template
cp .env.example .env
vim .env

# Key S2 settings:
LANGUAGE=bi                    # en, th, bi (bilingual)
MULTI_INSURER_ENABLED=true
INSURER_DEDUP_ENABLED=true
OCR_ENABLED=true
SYN_ENDPOINT=http://localhost:9002/ocr  # Syn service
THAI_NLP_ENDPOINT=http://localhost:9001 # Thai NER
```

### Step 3: Create Insurer Configuration (3 min)

```bash
# Create insurers.json
cat > config/insurers.json << 'EOF'
{
  "insurer_001": {
    "name": "Prudential Thailand",
    "language": "en",
    "urls": [
      "https://prudential.co.th/en/products/health/",
      "https://prudential.co.th/en/products/life/"
    ]
  },
  "insurer_002": {
    "name": "AXA Thailand",
    "language": "bi",
    "urls": [
      "https://axa.co.th/en/products/",
      "https://axa.co.th/th/products/"
    ]
  },
  "insurer_003": {
    "name": "Thai Health Insurance",
    "language": "th",
    "upload_dir": "./data/uploads/thai-health/"
  }
}
EOF
```

---

## 📋 Phase 1 (S2): Multi-Source Extraction

### Option A: Extract from URLs Only

```bash
python main.py --phase 1 \
  --insurers config/insurers.json \
  --language bi

# Output: 1000+ chunks from Prudential (EN) + AXA (EN+TH)
# Time: ~10 min
```

### Option B: Upload Files (PDF, DOCX, Images)

```bash
# Prepare uploads directory
mkdir -p data/uploads/prudential
cp documents/*.pdf data/uploads/prudential/

# Run Phase 1 with file processing
python main.py --phase 1 \
  --upload-dir data/uploads/ \
  --insurers config/insurers.json \
  --process-ocr

# Output: Chunks from documents + OCR text from images
# Time: ~15-20 min (depends on file size)
```

### Option C: URLs + Files (Hybrid)

```bash
# Extract from both URLs and uploaded files
python main.py --phase 1 \
  --insurers config/insurers.json \
  --upload-dir data/uploads/ \
  --language bi \
  --process-ocr

# Outputs:
# - phase1_chunks.jsonl (all chunks with insurer_id + language)
# - Time: ~20-30 min
```

### File Upload Support

```bash
# Supported formats:
# 📄 PDF:        .pdf (via pypdf)
# 📝 DOCX:       .docx, .doc (via python-docx)
# 📋 Text:       .txt (plain text)
# 🖼️  Images:    .jpg, .jpeg, .png (via Syn OCR or pytesseract)

# Example: Upload multiple insurer documents
ls data/uploads/prudential/*.pdf | head -3
# prudential_health_2026.pdf
# prudential_life_2026.pdf
# prudential_investment_2026.pdf

# Process them:
python main.py --phase 1 --upload-dir data/uploads/prudential/
```

---

## 🇹🇭 Thai Language Support

### Thai Preprocessing Pipeline

```python
# Phase 1 extracts Thai text, Phase 2 preprocesses:
language=th  →  pythainlp tokenization  →  NER extraction  →  Entity linking

# Supported NER entities (Thai):
# - Products (ประกันภัย)
# - Coverage types (ความคุ้มครอง)
# - Medical conditions (โรค)
# - Exclusions (ข้อยกเว้น)
```

### Run with Thai Documents

```bash
# 1. Upload Thai documents
mkdir data/uploads/thai-health/
cp thai_documents/*.pdf data/uploads/thai-health/

# 2. Configure Thai processing
python main.py --phase 1 \
  --upload-dir data/uploads/thai-health/ \
  --language th \
  --thai-tokenizer pythainlp

# 3. Phase 2 will detect Thai and apply Thai NER
python main.py --phase 2 \
  --language th

# 4. Phase 3 extracts Thai-specific entities
python main.py --phase 3 \
  --language th
```

### Bilingual (English + Thai)

```bash
# Extract both EN and TH from same source (if available)
python main.py --phase 1 \
  --insurers config/insurers.json \
  --language bi

# This creates:
# - "language": "en" chunks for English docs
# - "language": "th" chunks for Thai docs
# - Can query across both languages
```

---

## 📁 Multi-File Upload with Batch Processing

### Upload Structure

```
data/uploads/
├── prudential/
│   ├── health_insurance_2026.pdf
│   ├── life_insurance_2026.pdf
│   └── cover_page.png  (OCR extracted)
├── axa/
│   ├── products_catalog.docx
│   └── policy_samples/
│       ├── sample1.jpg
│       └── sample2.jpg
└── thai-health/
    ├── policy_guide.pdf
    ├── exclusions_thai.pdf
    └── comparison_chart.png
```

### Run Batch Upload

```bash
# Process all insurer uploads
python main.py --phase 1 \
  --upload-dir data/uploads/ \
  --process-ocr \
  --batch-size 10

# Progress:
# [1_extraction] Processing prudential/health_insurance_2026.pdf
# [1_extraction] ✅ Extracted 45 chunks (13,500 tokens)
# [1_extraction] Processing prudential/cover_page.png (OCR)
# [1_extraction] ✅ OCR extracted 12 chunks (3,600 tokens)
# ... (continue for all files)

# Output: phase1_chunks.jsonl with metadata:
# {
#   "insurer_id": "insurer_001",
#   "source_type": "file",
#   "file_name": "health_insurance_2026.pdf",
#   "language": "en",
#   ...
# }
```

---

## 📊 Monitoring Multi-Insurer Ingestion

### Dashboard Metrics

```markdown
| Metric | Target | Current | Status |
|--------|--------|---------|--------|
| Total Chunks | 2000 | 1850 | 🟡 |
| Prudential | 960 | 960 | ✅ |
| AXA | 600 | 550 | 🟡 |
| Thai Health | 440 | 340 | 🟡 |
| Image OCR Success | 95% | 92% | ✅ |
| Thai NER Accuracy | 85% | 83% | ✅ |
| Dedup Detected | <5% | 3% | ✅ |
```

### Check Insurer Breakdown

```bash
# Query chunks by insurer_id
python main.py --stats --by-insurer

# Output:
# Insurer Breakdown (Phase 1 output):
# insurer_001 (Prudential): 960 chunks, 284,800 tokens
# insurer_002 (AXA): 550 chunks, 165,000 tokens
# insurer_003 (Thai Health): 340 chunks, 102,000 tokens
```

---

## 🚨 Decision Gates (S2 Timeline)

### June 8 (End of Phase 2-3): Schema + Entity Check

```bash
python main.py --stats --by-insurer

# Check:
# ✅ All 21 metadata fields present (+ insurer_id, language)
# ✅ Thai entities extracted correctly
# ✅ Deduplication working (check duplicate detection rate)
# ✅ File metadata captured (file_name, file_size)
```

### June 13 (End of Phase 4): Hit Rate + OCR Quality

```bash
python main.py --phase 5

# Check:
# ✅ Hit Rate@3 ≥75% (English)
# ✅ Hit Rate@3 ≥70% (Thai) — may be lower due to embedding
# ✅ OCR confidence >80%
# ✅ Dedup accuracy >95%

# If OCR <80%:
# → Re-run Syn endpoint with better image quality
# → Or fallback to manual review
```

### June 20 (Final): Compliance + Go-Live

```bash
python main.py --phase 6  # Compliance checks

# Check:
# ✅ Coverage limits within OIC regulations
# ✅ No conflicting exclusions
# ✅ All products compliant
# ✅ Ready for production
```

---

## 🐛 Troubleshooting (S2)

### OCR Not Working

```bash
# Check Syn service
curl -X POST http://localhost:9002/ocr \
  -F "image=@test_image.jpg"

# If fails, fallback to pytesseract:
apt-get install tesseract-ocr
pip install pytesseract

# Re-run Phase 1 with fallback
python main.py --phase 1 --process-ocr --ocr-fallback
```

### Thai Language Detection Failed

```bash
# Verify Thai tokenizer
python -c "from pythainlp import word_tokenize; print(word_tokenize('สวัสดี'))"

# Install if missing
pip install pythainlp>=3.1.0

# Check Thai NLP endpoint
curl http://localhost:9001/tokenize \
  -d '{"text": "ประกันภัย"}'
```

### Insurer Deduplication Too Aggressive

```bash
# Check dedup threshold
vim config/dedup_config.json

# Typical similarity threshold: 0.95 (very similar)
# Lower it if too many products being marked duplicates

# Re-run Phase 2 with new threshold
python main.py --phase 2 --dedup-threshold 0.90
```

### PDF Extraction Missing Text

```bash
# Some PDFs are image-based (scanned)
# Use OCR for those instead:

python main.py --phase 1 \
  --upload-dir data/uploads/scanned/ \
  --process-ocr \
  --ocr-mode image-first  # Try OCR before text extraction
```

---

## 📁 S2 Directory Structure

```
insurance_ingestion_s2/
├── core.py                      # S2: +insurer_id, language, upload_dir
├── main.py                      # S2: +--upload-dir, --language flags
├── requirements.txt             # S2: +pythainlp, pypdf, python-docx, Pillow
│
├── phases/
│   ├── phase1_extraction_s2.py  # S2: NEW - extract_from_files() + OCR
│   ├── phase2_schema.py         # S2: Updated - dedup logic
│   ├── phase3_entities.py       # S2: Updated - Thai NER
│   ├── phase4_ingestion.py      # S1: No changes
│   ├── phase5_validation.py     # S1: No changes
│   └── phase6_compliance.py     # S2: NEW - regulatory checks
│
├── tests/
│   ├── fixtures/
│   │   ├── sample_data.py       # S1: Reuse
│   │   ├── sample_thai_data.py  # S2: NEW
│   │   └── sample_uploads/      # S2: NEW test files
│   │
│   └── unit/
│       ├── test_phase1_extraction_s2.py  # S2: NEW
│       ├── test_phase2_dedup.py          # S2: NEW
│       ├── test_phase3_thai_ner.py       # S2: NEW
│       └── test_phase6_compliance.py     # S2: NEW
│
├── config/
│   └── insurers.json            # S2: NEW - multi-insurer config
│
├── docs/
│   ├── CLAUDE_S2.md             # This file
│   ├── THAI_LANGUAGE_GUIDE.md   # S2: NEW
│   ├── FILE_UPLOAD_GUIDE.md     # S2: NEW
│   └── COMPLIANCE_RULES.md      # S2: NEW
│
└── data/
    ├── uploads/                 # S2: NEW - for file uploads
    │   ├── prudential/
    │   ├── axa/
    │   └── thai-health/
    └── output/
        └── phase1_chunks.jsonl  # Includes insurer_id, language, source_type
```

---

## 📚 Key References

- **S1 → S2 Reuse:** 65% code reuse (Phase 2-5 mostly unchanged)
- **New Files:** phase1_extraction_s2.py, phase6_compliance.py, Thai/QA modules
- **New Tests:** 15+ new unit tests for S2 features
- **Config:** insurers.json drives multi-insurer extraction

---

## 🎯 Success Criteria (June 21)

- [ ] 2000+ chunks from 3 insurers ingested
- [ ] Thai extraction working (100+ Thai chunks)
- [ ] File upload working (50+ documents processed)
- [ ] OCR working (confidence >80%)
- [ ] Insurer deduplication accurate (>95%)
- [ ] Hit Rate@3 ≥75% (English) / ≥70% (Thai)
- [ ] All Phase 6 compliance checks passing
- [ ] Zero compliance violations detected

---

**Last Updated:** 2026-05-16  
**Contact:** paripol@megawiz.co  
**Sprint:** S2 (June 1-21, 2026)
