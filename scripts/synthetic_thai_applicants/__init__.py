"""Synthetic Thai insurance applicant generator (W2.1b / I1 dataset).

Produces reproducible Thai-localized synthetic data for:
- Asgard-Underwriter regression suite (I1)
- Fraud detection benchmark (I2)
- Medical chart OCR benchmark (M3, via PDF renderer)
- Skuggi PII detection coverage (X1) — Thai citizen IDs + Thai names

NOT real data. NOT for clinical use. All names/IDs/diagnoses synthetic.
"""
__version__ = "0.1.0"
