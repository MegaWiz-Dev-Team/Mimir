"""Thai national ID (เลขประจำตัวประชาชน) — 13 digits with mod-11 checksum.

Format: D1-DDDD-DDDDD-DD-C where C is the checksum.

Checksum rule (per กรมการปกครอง):
    sum = sum(digit[i] * (13 - i)) for i in 0..11
    checksum = (11 - sum % 11) % 10

Reference: https://en.wikipedia.org/wiki/National_identification_number#Thailand
"""
from __future__ import annotations
import random


def compute_checksum(twelve_digits: str) -> int:
    """Compute the 13th checksum digit for the first 12 digits."""
    if len(twelve_digits) != 12 or not twelve_digits.isdigit():
        raise ValueError(f"need exactly 12 digits, got {twelve_digits!r}")
    total = sum(int(d) * (13 - i) for i, d in enumerate(twelve_digits))
    return (11 - total % 11) % 10


def is_valid(citizen_id: str) -> bool:
    """Validate a 13-digit Thai citizen ID."""
    digits = "".join(c for c in citizen_id if c.isdigit())
    if len(digits) != 13:
        return False
    return int(digits[12]) == compute_checksum(digits[:12])


def generate(rng: random.Random | None = None) -> str:
    """Generate a valid 13-digit Thai citizen ID (no formatting)."""
    rng = rng or random.Random()
    twelve = "".join(str(rng.randint(0, 9)) for _ in range(12))
    # First digit cannot be 0 (per format spec)
    while twelve[0] == "0":
        twelve = str(rng.randint(1, 9)) + twelve[1:]
    return twelve + str(compute_checksum(twelve))


def format_display(citizen_id: str) -> str:
    """Format as D-DDDD-DDDDD-DD-C for display."""
    d = "".join(c for c in citizen_id if c.isdigit())
    if len(d) != 13:
        return citizen_id
    return f"{d[0]}-{d[1:5]}-{d[5:10]}-{d[10:12]}-{d[12]}"
