# Entity Extraction Configuration
## From Peer Review - Add to scripts/extract_entities.py

**Status:** ✅ Peer review requirement (Data Engineer)  
**Priority:** HIGH (blocks S1.3)  
**Owner:** Data Engineer  
**Deadline:** May 17, 5:00 PM

---

## Configuration to Add

```python
# Add to scripts/extract_entities.py (top of file, after imports)

ENTITY_EXTRACTION_CONFIG = {
    # NER Models
    "models": {
        "english": {
            "library": "spacy",
            "model": "en_core_web_sm",
            "description": "English Named Entity Recognition"
        },
        "thai": {
            "library": "pythainlp",
            "model": "named_entity",
            "description": "Thai Named Entity Recognition"
        }
    },
    
    # Confidence Thresholds (per entity type)
    "confidence_thresholds": {
        "product": 0.85,       # High: Product names must be clear
        "coverage": 0.80,      # Medium-high: Coverage terms
        "exclusion": 0.75,     # Medium: Exclusion terms
        "condition": 0.70      # Medium-low: Age/condition requirements
    },
    
    # Expected output range
    "expected_entity_count": {
        "min": 350,
        "target": 500,
        "max": 700
    },
    
    # Quality assurance
    "quality_checks": {
        "sample_size": 50,              # Manually verify 50 entities
        "duplicate_check": True,         # Flag duplicate entities
        "confidence_avg_minimum": 0.72   # Average confidence target
    },
    
    # Fallback strategy (if entity count too low)
    "fallback": {
        "trigger": "entities < 350 OR avg_confidence < 0.72",
        "action": "MANUAL_REVIEW",
        "steps": [
            "1. Reduce confidence thresholds by 0.05",
            "2. Re-run entity extraction",
            "3. Manually validate new entities",
            "4. Proceed if entities >= 400"
        ]
    }
}

# Language detection helper
def detect_chunk_language(chunk_text):
    """
    Detect if chunk is Thai or English
    Returns: "th" or "en"
    """
    # Thai character ranges: U+0E00 to U+0E7F
    thai_chars = sum(1 for c in chunk_text if '฀' <= c <= '๿')
    thai_ratio = thai_chars / len(chunk_text) if chunk_text else 0
    return "th" if thai_ratio > 0.3 else "en"

# Extract entities with configured thresholds
def extract_entities(chunk_text, chunk_id):
    """
    Extract entities from chunk using language-specific model
    Returns: list of {entity, type, confidence, chunk_id}
    """
    language = detect_chunk_language(chunk_text)
    
    if language == "en":
        import spacy
        nlp = spacy.load(ENTITY_EXTRACTION_CONFIG["models"]["english"]["model"])
        doc = nlp(chunk_text)
    else:
        from pythainlp import ner
        entities = ner.tag(chunk_text)  # Returns [(word, tag), ...]
    
    extracted = []
    threshold = ENTITY_EXTRACTION_CONFIG["confidence_thresholds"]
    
    for entity in doc.ents if language == "en" else entities:
        # Determine entity type
        if language == "en":
            entity_type = entity.label_  # PERSON, ORG, GPE, etc.
            entity_text = entity.text
            confidence = 0.80  # spacy doesn't give per-entity confidence
        else:
            entity_text, entity_type = entity
            confidence = 0.75  # Thai NER baseline confidence
        
        # Map to our entity types
        mapped_type = map_to_entity_type(entity_type)
        
        # Check confidence threshold
        min_confidence = threshold.get(mapped_type, 0.70)
        if confidence >= min_confidence:
            extracted.append({
                "entity": entity_text,
                "type": mapped_type,
                "confidence": confidence,
                "chunk_id": chunk_id,
                "language": language
            })
    
    return extracted

def map_to_entity_type(spacy_label):
    """
    Map spaCy labels to our entity types
    """
    mapping = {
        "PRODUCT": "product",
        "ORG": "product",           # Organization = insurance product
        "GPE": "condition",         # Location = condition (e.g., "Thailand")
        "MONEY": "coverage",        # Money = coverage amount
        "PERSON": "exclusion",      # Skip person names (privacy)
        # Add more mappings as needed
    }
    return mapping.get(spacy_label, "condition")
```

---

## Testing the Config

```bash
# Test entity extraction with new config
python3 scripts/extract_entities.py \
  --input data/output/smoke_test_chunks.jsonl \
  --output data/output/test_entities.jsonl \
  --config CONFIG_ENTITY_EXTRACTION.md

# Expected output (sample):
# {
#   "chunk_id": "chunk_001",
#   "entities": [
#     {"entity": "PRU Mao Mao", "type": "product", "confidence": 0.92},
#     {"entity": "critical illness", "type": "coverage", "confidence": 0.88},
#     {"entity": "pre-existing", "type": "exclusion", "confidence": 0.85}
#   ],
#   "entity_count": 3,
#   "avg_confidence": 0.88
# }

# Check results
jq '.entity_count' data/output/test_entities.jsonl | sort | uniq -c
# Expected: Most chunks have 2-5 entities
```

---

## Implementation Checklist

- [ ] Copy config into scripts/extract_entities.py
- [ ] Install required: `pip install spacy pythainlp`
- [ ] Download English model: `python -m spacy download en_core_web_sm`
- [ ] Test on smoke test data
- [ ] Verify confidence thresholds work as expected
- [ ] Document any custom entity types found
- [ ] Commit: "Add entity extraction config with thresholds"

---

## S1.3 Quality Gate

**Before proceeding to Neo4j:**

```
☐ Entity count: 350-700 (target 500)
☐ Average confidence: ≥ 0.72
☐ Sample review: Manually check 50 entities
☐ No obvious false positives
☐ Language detection working (Thai vs English)
```

**If entity count < 350:**
- Use fallback: Reduce thresholds by 0.05
- Re-run extraction
- Manual review before proceeding

---

**Owner:** Data Engineer  
**Deadline:** May 17, 5:00 PM (for testing Monday)  
**Use in:** S1.3 (May 22-24)

