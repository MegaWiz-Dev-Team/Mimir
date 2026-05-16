"""S2 Sample data: Multi-insurer, Thai language, file uploads, OCR test data."""

from pathlib import Path
from insurance_ingestion_s2.core import Chunk, Entity

# ============================================================================
# SAMPLE CHUNKS (Multi-Insurer)
# ============================================================================

SAMPLE_CHUNKS_PRUDENTIAL = [
    Chunk(
        source_id="url_insurer_001_health_1",
        content=(
            "PRU Mao Mao Double Sure is a comprehensive health insurance plan "
            "that covers hospitalization up to THB 2,000,000 per year. "
            "Daily room benefit is THB 6,000. Includes surgical expenses, "
            "intensive care, and emergency outpatient coverage. "
            "No medical check-up required, just complete a health questionnaire."
        ),
        metadata={
            "source_url": "https://prudential.co.th/en/products/health/",
            "document_type": "product_catalog",
            "language": "en",
            "vendor": "VENDOR_INSURANCE_001",
            "source_type": "url",
            "insurer_id": "insurer_001",
        },
        chunk_index=0,
        tokens=156,
        insurer_id="insurer_001",
        language="en",
        source_type="url",
    ),
    Chunk(
        source_id="url_insurer_001_health_2",
        content=(
            "Exclusions: Cosmetic procedures, experimental treatments, "
            "dental care, pregnancy and childbirth related claims, "
            "pre-existing conditions within first 12 months. "
            "Claims must be submitted within 90 days of treatment completion."
        ),
        metadata={
            "source_url": "https://prudential.co.th/en/products/health/",
            "document_type": "product_catalog",
            "language": "en",
            "vendor": "VENDOR_INSURANCE_001",
            "source_type": "url",
            "insurer_id": "insurer_001",
        },
        chunk_index=1,
        tokens=89,
        insurer_id="insurer_001",
        language="en",
        source_type="url",
    ),
]

SAMPLE_CHUNKS_AXA_EN = [
    Chunk(
        source_id="url_insurer_002_health_1",
        content=(
            "AXA Health Plus offers comprehensive medical coverage "
            "with flexible deductible options (THB 5,000, 10,000, 25,000). "
            "Coverage includes hospitalization, surgery, and preventive care. "
            "24/7 customer support and easy claims through mobile app."
        ),
        metadata={
            "source_url": "https://axa.co.th/en/products/health/",
            "document_type": "product_catalog",
            "language": "en",
            "vendor": "VENDOR_INSURANCE_002",
            "source_type": "url",
            "insurer_id": "insurer_002",
        },
        chunk_index=0,
        tokens=102,
        insurer_id="insurer_002",
        language="en",
        source_type="url",
    ),
]

SAMPLE_CHUNKS_AXA_TH = [
    Chunk(
        source_id="url_insurer_002_health_th_1",
        content=(
            "แผน AXA สุขภาพ พลัส ให้ความคุ้มครองสำหรับการรักษาพยาบาลแบบครอบคลุม "
            "รวมถึงการนอนโรงพยาบาล การผ่าตัด และการดูแลเพื่อป้องกันโรค "
            "มีตัวเลือกการหักเงินเอาเองที่หลากหลาย ตั้งแต่ 5000 ถึง 25000 บาท"
        ),
        metadata={
            "source_url": "https://axa.co.th/th/products/health/",
            "document_type": "product_catalog",
            "language": "th",
            "vendor": "VENDOR_INSURANCE_002",
            "source_type": "url",
            "insurer_id": "insurer_002",
        },
        chunk_index=0,
        tokens=95,
        insurer_id="insurer_002",
        language="th",
        source_type="url",
    ),
]

SAMPLE_CHUNKS_THAI_HEALTH = [
    Chunk(
        source_id="file_insurer_003_health_policy_a1b2c3d4",
        content=(
            "บริษัท ประกันสุขภาพไทย มาตรฐาน ให้ความคุ้มครองโรคร้ายแรง "
            "รวม 63 โรค ตั้งแต่ราคาเพียง 2 บาทต่อวัน "
            "ความคุ้มครองจนถึง 99 ปี เหมาะสำหรับทุกเพศ ทุกวัย "
            "ฟรีอินเตอร์เน็ตคิดเห็นแพทย์และการส่งมอบสัตวแพทย์"
        ),
        metadata={
            "file_path": "data/uploads/thai-health/policy_2026.pdf",
            "file_name": "policy_2026.pdf",
            "document_type": "pdf",
            "language": "th",
            "vendor": "VENDOR_INSURANCE_003",
            "source_type": "upload",
            "insurer_id": "insurer_003",
        },
        chunk_index=0,
        tokens=121,
        insurer_id="insurer_003",
        language="th",
        source_type="upload",
    ),
]

SAMPLE_CHUNKS_TIPINSURE_EN = [
    Chunk(
        source_id="url_insurer_004_health_1",
        content=(
            "TipInsure provides comprehensive health insurance coverage for individuals and families "
            "in Thailand. Plans include outpatient treatment, hospitalization, and surgical expenses. "
            "Coverage up to THB 3,000,000 annually with low premiums and fast claim processing."
        ),
        metadata={
            "source_url": "https://www.tipinsure.com/en/products/",
            "document_type": "product_catalog",
            "language": "en",
            "vendor": "VENDOR_INSURANCE_004",
            "source_type": "url",
            "insurer_id": "insurer_004",
        },
        chunk_index=0,
        tokens=98,
        insurer_id="insurer_004",
        language="en",
        source_type="url",
    ),
]

SAMPLE_CHUNKS_TIPINSURE_TH = [
    Chunk(
        source_id="url_insurer_004_health_th_1",
        content=(
            "แผนประกันสุขภาพ TipInsure มีความคุ้มครองแบบครอบคลุมสำหรับการรักษาพยาบาล "
            "รวมถึงการนอนโรงพยาบาล การผ่าตัด และค่ารักษาพยาบาลนอกสถาบัน "
            "ความคุ้มครองสูงถึง 3 ล้านบาท ต่อปี โดยมีเบี้ยประกันที่เหมาะสม"
        ),
        metadata={
            "source_url": "https://www.tipinsure.com/th/products/",
            "document_type": "product_catalog",
            "language": "th",
            "vendor": "VENDOR_INSURANCE_004",
            "source_type": "url",
            "insurer_id": "insurer_004",
        },
        chunk_index=0,
        tokens=105,
        insurer_id="insurer_004",
        language="th",
        source_type="url",
    ),
]

SAMPLE_CHUNKS_THAILIFE_EN = [
    Chunk(
        source_id="url_insurer_005_life_1",
        content=(
            "Thai Life Insurance offers life insurance and investment-linked insurance products. "
            "Our comprehensive coverage includes term life, whole life, and endowment plans. "
            "With over 60 years of experience serving Thai families and businesses."
        ),
        metadata={
            "source_url": "https://www.thailife.com/en/products/",
            "document_type": "product_catalog",
            "language": "en",
            "vendor": "VENDOR_INSURANCE_005",
            "source_type": "url",
            "insurer_id": "insurer_005",
        },
        chunk_index=0,
        tokens=92,
        insurer_id="insurer_005",
        language="en",
        source_type="url",
    ),
]

SAMPLE_CHUNKS_THAILIFE_TH = [
    Chunk(
        source_id="url_insurer_005_life_th_1",
        content=(
            "บริษัท ประกันชีวิตไทย จำกัด มหาชน เสนอผลิตภัณฑ์ประกันชีวิต "
            "และประกันชีวิตลิงก์กับการลงทุน รวมถึงแผนประกันชีวิตระยะเวลา "
            "แบบเบ็ดเตล็ด และแบบการสะสมทุน ด้วยประสบการณ์กว่า 60 ปี"
        ),
        metadata={
            "source_url": "https://www.thailife.com/th/products/",
            "document_type": "product_catalog",
            "language": "th",
            "vendor": "VENDOR_INSURANCE_005",
            "source_type": "url",
            "insurer_id": "insurer_005",
        },
        chunk_index=0,
        tokens=88,
        insurer_id="insurer_005",
        language="th",
        source_type="url",
    ),
]

SAMPLE_CHUNKS_AIA_EN = [
    Chunk(
        source_id="url_insurer_006_health_1",
        content=(
            "AIA Thailand provides comprehensive life and health insurance solutions. "
            "Products include health insurance with hospital coverage up to THB 5,000,000, "
            "life insurance, and investment-linked plans. Fast claim settlement within 7 days."
        ),
        metadata={
            "source_url": "https://www.aia.co.th/en/products/",
            "document_type": "product_catalog",
            "language": "en",
            "vendor": "VENDOR_INSURANCE_006",
            "source_type": "url",
            "insurer_id": "insurer_006",
        },
        chunk_index=0,
        tokens=108,
        insurer_id="insurer_006",
        language="en",
        source_type="url",
    ),
]

SAMPLE_CHUNKS_AIA_TH = [
    Chunk(
        source_id="url_insurer_006_health_th_1",
        content=(
            "บริษัท เอไอเอ ไทย จำกัด มหาชน บริการประกันชีวิตและประกันสุขภาพอย่างครอบคลุม "
            "ผลิตภัณฑ์รวมถึงประกันสุขภาพการนอนโรงพยาบาลสูงถึง 5 ล้านบาท "
            "ประกันชีวิต และแผนลิงก์กับการลงทุน การชำระคำขอเรียกร้องรวดเร็ว"
        ),
        metadata={
            "source_url": "https://www.aia.co.th/th/products/",
            "document_type": "product_catalog",
            "language": "th",
            "vendor": "VENDOR_INSURANCE_006",
            "source_type": "url",
            "insurer_id": "insurer_006",
        },
        chunk_index=0,
        tokens=115,
        insurer_id="insurer_006",
        language="th",
        source_type="url",
    ),
]

SAMPLE_CHUNKS_BANGKOK_LIFE = [
    Chunk(
        source_id="url_insurer_007_life_1",
        content=(
            "Bangkok Life Insurance established 1947 offers traditional and modern life insurance products. "
            "Products include whole life, term life, endowment, and unit-linked insurance. Coverage from THB 100,000 to THB 10,000,000."
        ),
        metadata={
            "source_url": "https://www.bangkoklife.com/en/products/",
            "document_type": "product_catalog",
            "language": "en",
            "vendor": "VENDOR_INSURANCE_007",
            "source_type": "url",
            "insurer_id": "insurer_007",
        },
        chunk_index=0,
        tokens=110,
        insurer_id="insurer_007",
        language="en",
        source_type="url",
    ),
]

SAMPLE_CHUNKS_MUANG_THAI = [
    Chunk(
        source_id="url_insurer_008_life_1",
        content=(
            "Muang Thai Life Insurance offers comprehensive life insurance and investment products. "
            "Specializes in personal savings plans, group insurance, and critical illness coverage. "
            "Premium payment flexibility with monthly or annual options."
        ),
        metadata={
            "source_url": "https://www.muangthailife.com/en/products/",
            "document_type": "product_catalog",
            "language": "en",
            "vendor": "VENDOR_INSURANCE_008",
            "source_type": "url",
            "insurer_id": "insurer_008",
        },
        chunk_index=0,
        tokens=105,
        insurer_id="insurer_008",
        language="en",
        source_type="url",
    ),
]

SAMPLE_CHUNKS_KRUNGTHAI = [
    Chunk(
        source_id="url_insurer_009_insurance_1",
        content=(
            "Krungthai Insurance provides life and non-life insurance solutions backed by government support. "
            "Portfolio includes health insurance, life insurance, property insurance, and motor vehicle insurance. "
            "Trusted partner for individuals and businesses."
        ),
        metadata={
            "source_url": "https://www.krungthai.com/en/products/",
            "document_type": "product_catalog",
            "language": "en",
            "vendor": "VENDOR_INSURANCE_009",
            "source_type": "url",
            "insurer_id": "insurer_009",
        },
        chunk_index=0,
        tokens=108,
        insurer_id="insurer_009",
        language="en",
        source_type="url",
    ),
]

SAMPLE_CHUNKS_METLIFE = [
    Chunk(
        source_id="url_insurer_010_life_1",
        content=(
            "MetLife Thailand delivers innovative life insurance and retirement solutions. "
            "Products include term life, whole life, universal life, and variable universal life insurance. "
            "Integrated with healthcare and wellness programs for comprehensive protection."
        ),
        metadata={
            "source_url": "https://www.metlife.co.th/en/products/",
            "document_type": "product_catalog",
            "language": "en",
            "vendor": "VENDOR_INSURANCE_010",
            "source_type": "url",
            "insurer_id": "insurer_010",
        },
        chunk_index=0,
        tokens=112,
        insurer_id="insurer_010",
        language="en",
        source_type="url",
    ),
]

SAMPLE_CHUNKS_ALLIANZ = [
    Chunk(
        source_id="url_insurer_011_insurance_1",
        content=(
            "Allianz Ayudhya combines global expertise with local market knowledge. "
            "Offers life insurance, health insurance, and general insurance products. "
            "Claims settlement through network of hospitals and service centers nationwide."
        ),
        metadata={
            "source_url": "https://www.allianzayudhya.co.th/en/products/",
            "document_type": "product_catalog",
            "language": "en",
            "vendor": "VENDOR_INSURANCE_011",
            "source_type": "url",
            "insurer_id": "insurer_011",
        },
        chunk_index=0,
        tokens=105,
        insurer_id="insurer_011",
        language="en",
        source_type="url",
    ),
]

SAMPLE_CHUNKS_MANULIFE = [
    Chunk(
        source_id="url_insurer_012_life_1",
        content=(
            "Manulife Thailand offers wealth creation and protection solutions through diverse insurance products. "
            "Includes unit-linked insurance, life insurance, and critical illness coverage. "
            "Digital-first approach with online policy management and claims."
        ),
        metadata={
            "source_url": "https://www.manulife.co.th/en/products/",
            "document_type": "product_catalog",
            "language": "en",
            "vendor": "VENDOR_INSURANCE_012",
            "source_type": "url",
            "insurer_id": "insurer_012",
        },
        chunk_index=0,
        tokens=108,
        insurer_id="insurer_012",
        language="en",
        source_type="url",
    ),
]

SAMPLE_CHUNKS_PRINCIPAL = [
    Chunk(
        source_id="url_insurer_013_retirement_1",
        content=(
            "Principal Thailand specializes in retirement income and investment solutions. "
            "Products include pension plans, group insurance, and investment-linked insurance. "
            "Focus on helping customers achieve their financial goals through diversified portfolios."
        ),
        metadata={
            "source_url": "https://www.principal.co.th/en/products/",
            "document_type": "product_catalog",
            "language": "en",
            "vendor": "VENDOR_INSURANCE_013",
            "source_type": "url",
            "insurer_id": "insurer_013",
        },
        chunk_index=0,
        tokens=105,
        insurer_id="insurer_013",
        language="en",
        source_type="url",
    ),
]

SAMPLE_CHUNKS_GENERALI = [
    Chunk(
        source_id="url_insurer_014_insurance_1",
        content=(
            "Generali Thailand provides integrated insurance solutions combining life and general insurance. "
            "Comprehensive coverage for individuals and businesses including health, life, and property insurance. "
            "Customer-centric approach with simplified claims process and 24/7 support."
        ),
        metadata={
            "source_url": "https://www.generali.co.th/en/products/",
            "document_type": "product_catalog",
            "language": "en",
            "vendor": "VENDOR_INSURANCE_014",
            "source_type": "url",
            "insurer_id": "insurer_014",
        },
        chunk_index=0,
        tokens=110,
        insurer_id="insurer_014",
        language="en",
        source_type="url",
    ),
]

# ============================================================================
# SAMPLE ENTITIES (Multi-Insurer)
# ============================================================================

SAMPLE_ENTITIES = [
    Entity(
        entity_id="prod_insurer_001_health_01",
        name="PRU Mao Mao Double Sure",
        entity_type="Product",
        properties={
            "product_category": "Health Insurance",
            "insurer_id": "insurer_001",
            "coverage_type": "Comprehensive",
            "max_coverage_thb": 2000000,
            "room_benefit_daily_thb": 6000,
        },
        source_ids=["url_insurer_001_health_1", "url_insurer_001_health_2"],
    ),
    Entity(
        entity_id="prod_insurer_002_health_01",
        name="AXA Health Plus",
        entity_type="Product",
        properties={
            "product_category": "Health Insurance",
            "insurer_id": "insurer_002",
            "coverage_type": "Flexible",
            "deductible_options_thb": [5000, 10000, 25000],
        },
        source_ids=["url_insurer_002_health_1", "url_insurer_002_health_th_1"],
    ),
    Entity(
        entity_id="cov_critical_illness",
        name="Critical Illness Coverage",
        entity_type="Coverage",
        properties={
            "covered_conditions": ["cancer", "heart_disease", "stroke"],
            "common_in_insurers": ["insurer_001", "insurer_002", "insurer_003"],
        },
        source_ids=["url_insurer_001_health_1", "url_insurer_002_health_1", "file_insurer_003_health_policy_a1b2c3d4"],
    ),
    Entity(
        entity_id="excl_cosmetic",
        name="Cosmetic Procedures Exclusion",
        entity_type="Exclusion",
        properties={
            "excluded_services": ["cosmetic_surgery", "plastic_surgery"],
            "applies_to": ["insurer_001"],
        },
        source_ids=["url_insurer_001_health_2"],
    ),
]

# ============================================================================
# MULTI-INSURER TEST QUERIES
# ============================================================================

SAMPLE_TEST_QUERIES_EN = [
    {
        "query": "What health insurance plans cover critical illness?",
        "tier": "lookup",
        "min_hit_rate": 0.80,
        "expected_entities": ["Critical Illness Coverage", "PRU Mao Mao Double Sure", "AXA Health Plus"],
        "expected_insurers": ["insurer_001", "insurer_002"],
    },
    {
        "query": "Which insurers offer hospitalization coverage?",
        "tier": "lookup",
        "min_hit_rate": 0.75,
        "expected_entities": ["Hospitalization", "Health Insurance"],
        "expected_insurers": ["insurer_001", "insurer_002", "insurer_003"],
    },
    {
        "query": "What are the differences between Prudential and AXA health plans?",
        "tier": "reasoning",
        "min_hit_rate": 0.70,
        "expected_entities": ["PRU Mao Mao Double Sure", "AXA Health Plus"],
        "expected_insurers": ["insurer_001", "insurer_002"],
    },
    {
        "query": "Are cosmetic procedures covered by insurance?",
        "tier": "exclusion",
        "min_hit_rate": 0.65,
        "expected_entities": ["Cosmetic Procedures Exclusion"],
        "expected_insurers": ["insurer_001"],
    },
]

SAMPLE_TEST_QUERIES_TH = [
    {
        "query": "ความคุ้มครองโรคร้ายแรงมีกี่โรค",
        "tier": "lookup",
        "min_hit_rate": 0.75,
        "expected_entities": ["Critical Illness", "63 diseases"],
        "expected_insurers": ["insurer_003"],
    },
    {
        "query": "ประกันสุขภาพไทยสามารถเลือกความคุ้มครองใดได้บ้าง",
        "tier": "lookup",
        "min_hit_rate": 0.70,
        "expected_entities": ["Coverage Options", "Flexible"],
        "expected_insurers": ["insurer_002", "insurer_003"],
    },
    {
        "query": "โรคไหนที่ไม่ได้รับความคุ้มครองตามนโยบาย",
        "tier": "exclusion",
        "min_hit_rate": 0.60,
        "expected_entities": ["Exclusion", "Not covered"],
        "expected_insurers": ["insurer_001"],
    },
]

# ============================================================================
# SAMPLE UPLOAD FILES (for Phase 1 testing)
# ============================================================================

SAMPLE_UPLOAD_FILES = [
    {
        "filename": "prudential_health_2026.pdf",
        "file_type": "pdf",
        "insurer_id": "insurer_001",
        "expected_chunks": 5,
        "content_snippet": "Hospitalization coverage up to THB 2M",
    },
    {
        "filename": "axa_bilingual_products.docx",
        "file_type": "docx",
        "insurer_id": "insurer_002",
        "expected_chunks": 3,
        "content_snippet": "Health Plus, flexible deductible",
    },
    {
        "filename": "thai_health_policy_scan.jpg",
        "file_type": "image",
        "insurer_id": "insurer_003",
        "expected_chunks": 2,
        "ocr_required": True,
        "content_snippet": "63 โรค ประกันชีวิต",
    },
]

# ============================================================================
# INSURER CONFIG (S2 Feature)
# ============================================================================

SAMPLE_INSURERS_CONFIG = {
    "insurer_001": {
        "name": "Prudential Thailand",
        "language": "en",
        "urls": [
            "https://prudential.co.th/en/products/health/",
            "https://prudential.co.th/en/products/life/",
        ],
    },
    "insurer_002": {
        "name": "AXA Thailand",
        "language": "bi",  # bilingual
        "urls": [
            "https://axa.co.th/en/products/health/",
            "https://axa.co.th/th/products/health/",
        ],
    },
    "insurer_003": {
        "name": "Thai Health Insurance",
        "language": "th",
        "upload_dir": "./data/uploads/thai-health/",
    },
    "insurer_004": {
        "name": "TipInsure Thailand",
        "language": "bi",  # bilingual Thai + English
        "urls": [
            "https://www.tipinsure.com/en/products/",
            "https://www.tipinsure.com/th/products/",
        ],
    },
    "insurer_005": {
        "name": "Thai Life Insurance",
        "language": "bi",  # bilingual Thai + English
        "urls": [
            "https://www.thailife.com/en/products/",
            "https://www.thailife.com/th/products/",
        ],
    },
    "insurer_006": {
        "name": "AIA Thailand",
        "language": "bi",  # bilingual Thai + English
        "urls": [
            "https://www.aia.co.th/en/products/",
            "https://www.aia.co.th/th/products/",
        ],
    },
    "insurer_007": {
        "name": "Bangkok Life Insurance",
        "language": "bi",
        "urls": [
            "https://www.bangkoklife.com/en/products/",
            "https://www.bangkoklife.com/th/products/",
        ],
    },
    "insurer_008": {
        "name": "Muang Thai Life Insurance",
        "language": "bi",
        "urls": [
            "https://www.muangthailife.com/en/products/",
            "https://www.muangthailife.com/th/products/",
        ],
    },
    "insurer_009": {
        "name": "Krungthai Insurance",
        "language": "bi",
        "urls": [
            "https://www.krungthai.com/en/products/",
            "https://www.krungthai.com/th/products/",
        ],
    },
    "insurer_010": {
        "name": "MetLife Thailand",
        "language": "bi",
        "urls": [
            "https://www.metlife.co.th/en/products/",
            "https://www.metlife.co.th/th/products/",
        ],
    },
    "insurer_011": {
        "name": "Allianz Ayudhya",
        "language": "bi",
        "urls": [
            "https://www.allianzayudhya.co.th/en/products/",
            "https://www.allianzayudhya.co.th/th/products/",
        ],
    },
    "insurer_012": {
        "name": "Manulife Thailand",
        "language": "bi",
        "urls": [
            "https://www.manulife.co.th/en/products/",
            "https://www.manulife.co.th/th/products/",
        ],
    },
    "insurer_013": {
        "name": "Principal Thailand",
        "language": "bi",
        "urls": [
            "https://www.principal.co.th/en/products/",
            "https://www.principal.co.th/th/products/",
        ],
    },
    "insurer_014": {
        "name": "Generali Thailand",
        "language": "bi",
        "urls": [
            "https://www.generali.co.th/en/products/",
            "https://www.generali.co.th/th/products/",
        ],
    },
}

# ============================================================================
# DEDUPLICATION TEST CASES
# ============================================================================

SAMPLE_DEDUP_CASES = [
    {
        "chunk_1": "PRU Mao Mao Double Sure covers hospitalization up to THB 2M",
        "chunk_2": "Prudential Mao Mao Double Sure provides hospitalization coverage THB 2 million",
        "similarity": 0.96,  # Should flag as potential duplicate
        "insurer_1": "insurer_001",
        "insurer_2": "insurer_001",  # Same insurer
        "action": "merge_or_flag",
    },
    {
        "chunk_1": "AXA Health Plus with THB 5000 deductible option",
        "chunk_2": "AXA Health Plus deductible can be THB 5k, 10k or 25k",
        "similarity": 0.92,
        "insurer_1": "insurer_002",
        "insurer_2": "insurer_002",
        "action": "merge_or_flag",
    },
    {
        "chunk_1": "Health insurance coverage for critical illness",
        "chunk_2": "Critical illness insurance protection plan",
        "similarity": 0.85,
        "insurer_1": "insurer_001",
        "insurer_2": "insurer_002",
        "action": "cross_insurer_comparison",  # Different insurers, no flag
    },
]

# ============================================================================
# THAI NER TEST DATA
# ============================================================================

SAMPLE_THAI_NER = [
    {
        "text": "ประกันภัยสุขภาพ PRU Mao Mao Double Sure ให้ความคุ้มครองโรคร้ายแรง 63 โรค",
        "expected_entities": [
            {"text": "ประกันภัยสุขภาพ", "type": "coverage"},
            {"text": "PRU Mao Mao Double Sure", "type": "product"},
            {"text": "โรคร้ายแรง", "type": "medical_condition"},
            {"text": "63", "type": "quantity"},
        ],
    },
    {
        "text": "มะเร็ง โรคหัวใจ และโรคหลอดเลือดสมองเป็นโรคร้ายแรงที่ปกป้อง",
        "expected_entities": [
            {"text": "มะเร็ง", "type": "medical_condition"},
            {"text": "โรคหัวใจ", "type": "medical_condition"},
            {"text": "โรคหลอดเลือดสมอง", "type": "medical_condition"},
        ],
    },
]

# ============================================================================
# OCR TEST DATA
# ============================================================================

SAMPLE_OCR_TEST_CASES = [
    {
        "image_file": "thai_policy_scan.jpg",
        "expected_text": "ประกันสุขภาพไทย",
        "expected_confidence": 0.92,
        "language": "th",
    },
    {
        "image_file": "prudential_brochure.png",
        "expected_text": "Hospitalization coverage THB",
        "expected_confidence": 0.88,
        "language": "en",
    },
    {
        "image_file": "blurry_scan.jpg",
        "expected_text": "unclear text",
        "expected_confidence": 0.65,  # Low confidence, should flag
        "language": "th",
        "action": "flag_for_manual_review",
    },
]
