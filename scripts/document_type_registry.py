#!/usr/bin/env python3
"""Asgard document-type registry: resolve any hospital's document name -> Asgard canonical type.

Each hospital / HIS calls documents differently. This binds every Thai / English /
abbreviation / alias to ONE Asgard canonical key + ONE FHIR R5 resource, with the
FHIR R5 structure as the source of truth.

Usage:
    python3 scripts/document_type_registry.py --render            # write DOCUMENT_TYPE_MAP.md
    python3 scripts/document_type_registry.py --resolve "ใบ order"  # -> order.medication
    python3 scripts/document_type_registry.py --resolve "CXR" "D/C summary" "ผลเพาะเชื้อ"
"""
from __future__ import annotations

import argparse
import json
import re
import sys
import unicodedata
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REGISTRY = ROOT / "data" / "abb" / "registry" / "document_types.json"
OUT_MD = ROOT / "data" / "abb" / "registry" / "DOCUMENT_TYPE_MAP.md"


def norm(s: str) -> str:
    """Lowercase, strip accents/punctuation/space — for tolerant alias matching."""
    s = unicodedata.normalize("NFKC", s or "").lower().strip()
    return re.sub(r"[\s\-_/.()]+", "", s)


def load():
    return json.loads(REGISTRY.read_text())


def build_index(data):
    """alias(normalized) -> asgard key. Later collisions are reported, first wins."""
    idx, collisions = {}, []
    for t in data["types"]:
        names = [t["asgard"]] + t.get("th", []) + t.get("en", []) + t.get("abbr", [])
        for n in names:
            k = norm(n)
            if not k:
                continue
            if k in idx and idx[k] != t["asgard"]:
                collisions.append((n, idx[k], t["asgard"]))
                continue
            idx[k] = t["asgard"]
    return idx, collisions


def resolve(query: str, data=None, idx=None):
    data = data or load()
    if idx is None:
        idx, _ = build_index(data)
    k = norm(query)
    # exact alias hit
    if k in idx:
        return idx[k], "exact"
    # substring fallback — prefer the LONGEST matching alias so short abbreviations
    # (e.g. "MAR" inside "sum-mar-y") never beat a longer, more specific alias.
    best = None  # (alias_len, key)
    for alias, key in idx.items():
        if len(alias) >= 4 and (alias in k or k in alias):
            if best is None or len(alias) > best[0]:
                best = (len(alias), key)
    if best:
        return best[1], "fuzzy"
    return None, "unmatched"


def render_md(data) -> str:
    L = [
        "# Asgard Document-Type Map (FHIR R5 anchored)",
        "",
        f"_version {data['version']} — {data['anchor']}_",
        "",
        "ชื่อกลางของ Asgard (`asgard`) + โครงสร้าง FHIR R5 เป็นแกนหลัก; ชื่อไทย/อังกฤษ/ตัวย่อ และ alias ที่แต่ละ รพ. เรียกต่างกัน ถูก map เข้าหาแกนเดียวกัน",
        "",
        "| Asgard canonical | FHIR R5 | LOINC | ชื่อไทย (รวม alias) | English | ตัวย่อ |",
        "|---|---|:---:|---|---|---|",
    ]
    for t in data["types"]:
        th = " · ".join(t.get("th", []))
        en = " · ".join(t.get("en", []))
        ab = " · ".join(t.get("abbr", [])) or "—"
        loinc = t["loinc"] or "—"
        L.append(f"| `{t['asgard']}` | `{t['fhir']}` | {loinc} | {th} | {en} | {ab} |")
    L += [
        "",
        "## หมวด (category)",
        "",
    ]
    cats = {}
    for t in data["types"]:
        cats.setdefault(t["category"], []).append(t["asgard"])
    for c, keys in cats.items():
        L.append(f"- **{c}**: " + ", ".join(f"`{k}`" for k in keys))
    L += [
        "",
        "## การใช้งาน (normalize ชื่อเอกสารจาก รพ. ใด ๆ)",
        "",
        "```bash",
        'python3 scripts/document_type_registry.py --resolve "ใบ D/C summary" "ผลเพาะเชื้อ" "CXR"',
        "```",
        "",
        "> `LOINC = —` คือยังไม่ผูกรหัส LOINC (เป็น resource เชิงโครงสร้าง เช่น Patient/Encounter/Coverage/Claim, หรือเอกสารที่ใช้แบบฟอร์ม MOPH เฉพาะ เช่น ใบรับรองแพทย์)",
        "",
    ]
    return "\n".join(L) + "\n"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--render", action="store_true", help="write DOCUMENT_TYPE_MAP.md")
    ap.add_argument("--resolve", nargs="+", metavar="NAME", help="resolve names to Asgard keys")
    args = ap.parse_args()

    data = load()
    idx, collisions = build_index(data)
    if collisions:
        print("⚠️  alias collisions (first wins):", file=sys.stderr)
        for name, a, b in collisions:
            print(f"   '{name}': {a} vs {b}", file=sys.stderr)

    if args.render:
        OUT_MD.write_text(render_md(data))
        print(f"rendered {OUT_MD}  ({len(data['types'])} types)")

    if args.resolve:
        fhir_by_key = {t["asgard"]: t["fhir"] for t in data["types"]}
        for q in args.resolve:
            key, how = resolve(q, data, idx)
            if key:
                print(f"  {q!r:<28} -> {key:<28} ({fhir_by_key[key]})  [{how}]")
            else:
                print(f"  {q!r:<28} -> UNMATCHED")

    if not args.render and not args.resolve:
        ap.print_help()


if __name__ == "__main__":
    main()
