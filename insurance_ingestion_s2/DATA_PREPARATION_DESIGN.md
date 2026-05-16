# Insurance S2: Data Preparation & Consolidation Design

**Goal:** Extract, consolidate, enrich metadata, then feed into chunking pipeline

---

## 📊 Data Sources Overview

### Source 1: Existing PDFs (992 KB, Thai)
```
/data/insurance/
├── PRUMhaoMhaoDoubleSure.pdf               (128 KB)
├── ข้อยกเว้นทั่วไปของกรมธรรม์.pdf         (120 KB)
├── รายละเอียดสัญญากรมธรรม์.pdf             (484 KB)
├── เงื่อนไขการรับประกัน.pdf                 (64 KB)
└── เงื่อนไขทั่วไปแห่งกรมธรรม์ประกันชีวิต.pdf (196 KB)
```

### Source 2: Web Pages (Marketing, English)
```
Primary URLs (3):
├── https://prudential.co.th/en/products/health/
├── https://prudential.co.th/en/products/life/
└── https://prudential.co.th/en/products/savings/

Deep URLs (7):
├── https://prudential.co.th/en/products/health/critical-illness/
├── https://prudential.co.th/en/products/health/explore-all/
├── https://prudential.co.th/en/products/life/accident/
├── https://prudential.co.th/en/products/life/explore-all/
├── https://prudential.co.th/en/products/savings/annuity/
├── https://prudential.co.th/en/products/savings/endowment/
├── https://prudential.co.th/en/products/savings/explore-all/
└── https://prudential.co.th/en/products/group-employee-plan/
```

---

## 🏗️ Data Preparation Pipeline

```
┌─────────────────────────────────────────────────────────────┐
│ STAGE 1: EXTRACT & NORMALIZE                                │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│ PDFs → OCR/Extract Text                                     │
│   ├─ Extract raw text + metadata (author, date, etc.)      │
│   └─ Detect language (Thai/English)                         │
│                                                               │
│ Web Pages → Scrape & Clean                                  │
│   ├─ Fetch HTML → Parse with BeautifulSoup                │
│   ├─ Remove navigation/boilerplate                          │
│   └─ Extract structured sections                            │
│                                                               │
└─────────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────────┐
│ STAGE 2: CATEGORIZE & ENRICH                                │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│ Detect Document Type:                                        │
│   ├─ PRODUCT_OVERVIEW   (web marketing pages)              │
│   ├─ PRODUCT_DETAIL     (specific product pages)           │
│   ├─ TERMS_CONDITIONS   (PDF policy documents)             │
│   ├─ EXCLUSIONS         (PDF general exclusions)           │
│   └─ COVERAGE_DETAILS   (PDF coverage conditions)          │
│                                                               │
│ Extract Key Metadata:                                        │
│   ├─ product_name       → "PRU Mao Mao Double Sure"        │
│   ├─ product_type       → "health" | "life" | "savings"    │
│   ├─ language           → "en" | "th" | "bi"               │
│   ├─ insurer_id         → "insurer_001" (Prudential)       │
│   ├─ document_type      → type detected above              │
│   ├─ source_url/path    → where it came from               │
│   ├─ extract_date       → when extracted                    │
│   └─ reliability_score  → high (official) / medium / low    │
│                                                               │
└─────────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────────┐
│ STAGE 3: CONSOLIDATE & DEDUPLICATE                          │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│ Merge Equivalent Content:                                    │
│   ├─ Web marketing + PDFs about same product → merge        │
│   ├─ Detect cross-references (e.g., "see terms" links)     │
│   └─ Flag redundant info (coverage mentioned in 2+ docs)   │
│                                                               │
│ Create Consolidated Records:                                │
│   ├─ product_id (canonical)                                │
│   ├─ consolidated_name (normalized product name)            │
│   ├─ all_sources[] (list of URLs/PDFs that mention it)     │
│   ├─ content_by_type {} (OVERVIEW, TERMS, EXCLUSIONS...)   │
│   └─ priority_rank (which source is most authoritative)     │
│                                                               │
└─────────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────────┐
│ STAGE 4: OUTPUT CONSOLIDATED DATASET                        │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│ Create: consolidated_products.jsonl                          │
│                                                               │
│ Format:                                                      │
│ {                                                            │
│   "product_id": "pru-mao-mao-001",                          │
│   "product_name": "PRU Mao Mao Double Sure",                │
│   "product_type": "health",                                 │
│   "insurer_id": "insurer_001",                              │
│   "language": "bi",  ← bilingual (Thai + English)          │
│   "content": {                                               │
│     "overview": "...[marketing description]...",            │
│     "coverage": "...[from PDFs]...",                        │
│     "exclusions": "...[from PDF exclusions]...",            │
│     "terms": "...[general terms]..."                        │
│   },                                                         │
│   "metadata": {                                              │
│     "document_type": "PRODUCT_DETAIL",                      │
│     "source_urls": [                                         │
│       "https://prudential.co.th/.../health/",              │
│       "https://prudential.co.th/.../health/explore-all/"   │
│     ],                                                       │
│     "source_pdfs": [                                         │
│       "PRUMhaoMhaoDoubleSure.pdf",                          │
│       "เงื่อนไขการรับประกัน.pdf"                           │
│     ],                                                       │
│     "extract_date": "2026-05-16",                           │
│     "reliability_score": 0.95,                              │
│     "is_bilingual": true,                                   │
│     "thai_percentage": 0.40,                                │
│     "english_percentage": 0.60                              │
│   },                                                         │
│   "quality_flags": {                                         │
│     "needs_review": false,                                  │
│     "has_duplicates": true,                                 │
│     "coverage_complete": true,                              │
│     "issues": []                                            │
│   }                                                          │
│ }                                                            │
│                                                               │
└─────────────────────────────────────────────────────────────┘
```

---

## 📋 Metadata Schema

### Core Fields
```python
{
  # Identity
  "product_id": str,           # "pru-mao-mao-001"
  "product_name": str,         # "PRU Mao Mao Double Sure"
  "product_version": str,      # "2.0" (if known)
  
  # Classification
  "product_type": str,         # "health" | "life" | "savings" | "investment"
  "insurer_id": str,           # "insurer_001"
  "channel": str,              # "direct" | "uob" | "ttb" (if applicable)
  "language": str,             # "en" | "th" | "bi"
  
  # Content
  "content": {
    "overview": str,           # Marketing/product overview
    "coverage": str,           # What's covered
    "exclusions": str,         # What's NOT covered
    "terms": str,              # General terms
    "premiums": str,           # If available
    "benefits": str            # Benefits list
  },
  
  # Source Tracking
  "metadata": {
    "document_type": str,      # "PRODUCT_OVERVIEW" | "TERMS_CONDITIONS" | etc.
    "source_urls": [str],      # ["https://...", "https://..."]
    "source_pdfs": [str],      # ["file.pdf", ...]
    "extract_date": str,       # ISO date
    "extract_method": str,     # "pdf_ocr" | "web_scrape" | "manual"
    "reliability_score": float # 0.0-1.0 (authoritativeness)
    "is_bilingual": bool,
    "thai_percentage": float,
    "english_percentage": float
  },
  
  # Quality
  "quality_flags": {
    "needs_review": bool,       # Human review needed?
    "has_duplicates": bool,     # Same product mentioned multiple times?
    "coverage_complete": bool,  # All sections have content?
    "issues": [str]             # ["OCR error line 42", ...]
  },
  
  # Temporal (for future filtering)
  "product_launch_date": str,  # "2020-01-15"
  "product_end_date": null,    # null = active, or ISO date
  "status": str                # "active" | "discontinued"
}
```

---

## 🔄 Implementation Steps

### Step 1: PDF Extraction
```bash
# Extract from PDFs → text files with metadata
→ Creates: data/extracted/pdf/
   ├── PRUMhaoMhaoDoubleSure.txt
   ├── ข้อยกเว้นทั่วไปของกรมธรรม์.txt
   └── ... (with .meta.json for each)
```

### Step 2: Web Scraping & Cleaning
```bash
# Fetch URLs → clean HTML → extract sections
→ Creates: data/extracted/web/
   ├── prudential_health_overview.txt
   ├── prudential_life_detail.txt
   └── ... (with .meta.json for each)
```

### Step 3: Categorization & Enrichment
```bash
# Analyze each document
# Detect: product name, type, language
# Extract: key sections
# Create metadata

→ Creates: data/categorized/
   ├── overview/
   ├── terms/
   ├── exclusions/
   └── details/
```

### Step 4: Consolidation
```bash
# Merge by product
# Combine content sections
# Deduplicate
# Create: consolidated_products.jsonl

→ Creates: data/consolidated/
   └── consolidated_products.jsonl (one product per line)
```

### Step 5: Feed to Chunking Pipeline
```bash
# consolidated_products.jsonl → Phase 1 extraction
# (existing chunking logic works with this format)

→ Output: phase1_chunks.jsonl (ready for Mimir ingestion)
```

---

## 🎯 Expected Output Example

```json
{
  "product_id": "pru-mao-mao-001",
  "product_name": "PRU Mao Mao Double Sure",
  "product_type": "health",
  "insurer_id": "insurer_001",
  "language": "bi",
  "content": {
    "overview": "Inpatient coverage up to 2 million baht per year, with room benefits of 6,000 baht/day. Easy application with no medical check-up, just a simple health questionnaire.",
    "coverage": "โครงสร้างการคุ้มครองประกอบด้วย: ค่าห้องพยาบาล ค่ารักษาต่างๆ ... [Thai terms]",
    "exclusions": "ไม่คุ้มครอง: โรคประจำตัวที่มีอยู่แต่ไม่เปิดเผย ... [Thai exclusions]",
    "terms": "General policy terms and conditions..."
  },
  "metadata": {
    "document_type": "PRODUCT_DETAIL",
    "source_urls": [
      "https://prudential.co.th/en/products/health/",
      "https://prudential.co.th/en/products/health/explore-all/"
    ],
    "source_pdfs": [
      "PRUMhaoMhaoDoubleSure.pdf",
      "เงื่อนไขการรับประกัน.pdf"
    ],
    "extract_date": "2026-05-16",
    "reliability_score": 0.95,
    "is_bilingual": true,
    "thai_percentage": 0.45,
    "english_percentage": 0.55
  },
  "quality_flags": {
    "needs_review": false,
    "has_duplicates": false,
    "coverage_complete": true,
    "issues": []
  },
  "product_launch_date": "2020-01-15",
  "product_end_date": null,
  "status": "active"
}
```

---

## 📁 Directory Structure (After Prep)

```
insurance_ingestion_s2/
├── data/
│   ├── raw/                          # Original files
│   │   ├── pdf/
│   │   │   └── [*.pdf files]
│   │   └── web/
│   │       └── [*.html files]
│   │
│   ├── extracted/                    # Raw text + metadata
│   │   ├── pdf/
│   │   │   ├── file1.txt
│   │   │   └── file1.meta.json
│   │   └── web/
│   │       ├── page1.txt
│   │       └── page1.meta.json
│   │
│   ├── categorized/                  # Labeled by type
│   │   ├── overview/
│   │   ├── terms/
│   │   ├── exclusions/
│   │   └── details/
│   │
│   ├── consolidated/                 # Final consolidated data
│   │   ├── consolidated_products.jsonl
│   │   └── consolidation_report.md
│   │
│   └── output/                       # Pipeline output (existing)
│       ├── phase1_chunks.jsonl
│       └── ...
```

---

## ✅ Data Quality Checklist

- [ ] All PDFs successfully extracted (no OCR failures)
- [ ] All web pages scraped without errors
- [ ] Metadata fields complete for all products
- [ ] No duplicate products in consolidated data
- [ ] Bilingual content properly tagged
- [ ] Product names normalized (spelling, case)
- [ ] Reliability scores assigned based on source authority
- [ ] Quality flags reviewed and accurate
- [ ] Ready for chunking pipeline

---

## 🚀 Ready for Phase 1?

Once `consolidated_products.jsonl` is ready:
1. Pipe directly to Phase 1 extraction
2. Existing chunking logic handles it
3. Chunks include full metadata (product_type, language, etc.)
4. Ready for Mimir ingestion with proper tagging

