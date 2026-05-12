//! 🌑 Skuggi text-PII benchmark — Rust port of `Syn/benchmarks/pii_bench.py`.
//!
//! Loads a gold JSON corpus + runs every case through `skuggi-core`'s
//! Tier 1 detectors, reports per-category precision/recall/F1. Single
//! source of truth = `skuggi-core` (Heimdall redaction, Mimir scoring,
//! and this benchmark all use the same regex set).
//!
//! Why Rust port?
//!   - Python version (`pii_bench.py`) carried its own regex copy, which
//!     diverged from the Heimdall regex over time (Tier 1b anchored
//!     patterns weren't there). The Rust port imports skuggi-core
//!     directly — drift is impossible.
//!   - Builds in CI (`cargo test`) without a Python runtime.
//!   - Cleaner for future enhancements (span-level F1, per-tenant gold
//!     comparison, etc.).
//!
//! Gold JSON shape (matches `Syn/benchmarks/build_medical_certs_gt.py`
//! output):
//!
//! ```jsonc
//! {
//!   "label_set": ["PATIENT_NAME", "DOCTOR_NAME", "HN", "LICENSE_NO", "THAI_ID"],
//!   "items": [
//!     {
//!       "id": "medcert-t001",
//!       "raw_text": "<the prompt with PII embedded>",
//!       "pii_types_present": ["PATIENT_NAME", "DOCTOR_NAME", "HN", "LICENSE_NO"]
//!     },
//!     ...
//!   ]
//! }
//! ```
//!
//! ## Usage
//!
//! ```sh
//! # Default gold path
//! cargo run --bin skuggi-bench
//! # Explicit gold path + threshold + case-by-case
//! cargo run --bin skuggi-bench -- \
//!     --gold ../../Syn/benchmarks/pii_gold_medical_certs.json \
//!     --threshold 0.6 \
//!     --case-by-case
//! ```
//!
//! ## Exit codes
//!   - `0` — every labelled category cleared the F1 threshold
//!   - `1` — at least one category below threshold (regex needs tuning)
//!   - `2` — gold file missing / unreadable / schema mismatch

use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::process::ExitCode;

// ─── Gold schema ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct GoldDoc {
    #[serde(default)]
    label_set: Vec<String>,
    items: Vec<GoldItem>,
}

#[derive(Debug, Deserialize)]
struct GoldItem {
    id: String,
    raw_text: String,
    pii_types_present: Vec<String>,
}

// ─── CLI args ────────────────────────────────────────────────────────────

#[derive(Debug)]
struct Args {
    gold: String,
    threshold: f64,
    case_by_case: bool,
}

fn parse_args() -> Args {
    let argv: Vec<String> = std::env::args().collect();
    let mut gold = "Syn/benchmarks/pii_gold_medical_certs.json".to_string();
    let mut threshold = 0.6_f64;
    let mut case_by_case = false;
    let mut i = 1;
    while i < argv.len() {
        match argv[i].as_str() {
            "--gold" if i + 1 < argv.len() => { gold = argv[i + 1].clone(); i += 2; }
            "--threshold" if i + 1 < argv.len() => {
                threshold = argv[i + 1].parse().unwrap_or(0.6);
                i += 2;
            }
            "--case-by-case" => { case_by_case = true; i += 1; }
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            _ => { i += 1; }
        }
    }
    Args { gold, threshold, case_by_case }
}

fn print_help() {
    eprintln!("🌑 skuggi-bench — Tier 1 PII detector benchmark");
    eprintln!();
    eprintln!("USAGE: skuggi-bench [--gold PATH] [--threshold F] [--case-by-case]");
    eprintln!();
    eprintln!("OPTIONS:");
    eprintln!("  --gold PATH       Path to gold JSON (default: Syn/benchmarks/pii_gold_medical_certs.json)");
    eprintln!("  --threshold F     Min F1 per category to exit 0 (default 0.6)");
    eprintln!("  --case-by-case    Print per-case predictions");
    eprintln!();
    eprintln!("EXIT CODES:");
    eprintln!("  0 = all categories ≥ F1 threshold");
    eprintln!("  1 = at least one category below threshold");
    eprintln!("  2 = gold file missing / unreadable");
}

// ─── Label normalisation ─────────────────────────────────────────────────
//
// The gold uses uppercase short labels (PATIENT_NAME, THAI_ID …) while
// skuggi-core emits its own category strings (patient_name,
// thai_id_anchored, thai_national_id, …). Map between them for scoring.
//
// THAI_ID gold → matches either anchored (`thai_id_anchored`) or free
// text (`thai_national_id`) — both are valid detections. Mapping reflects
// that: we score the union.

fn gold_to_skuggi(gold_label: &str) -> Vec<&'static str> {
    match gold_label {
        "PATIENT_NAME" => vec!["patient_name"],
        "DOCTOR_NAME" => vec!["doctor_name"],
        "HN" => vec!["hn"],
        "LICENSE_NO" => vec!["license_no"],
        "THAI_ID" => vec!["thai_id_anchored", "thai_national_id"],
        "PHONE" => vec!["thai_phone"],
        "EMAIL" => vec!["email"],
        _ => vec![],
    }
}

// ─── Metrics ────────────────────────────────────────────────────────────

#[derive(Default, Debug)]
struct CatStats { tp: u32, fp: u32, fn_: u32 }

fn safe_div(num: u32, den: u32) -> f64 {
    if den == 0 { 0.0 } else { num as f64 / den as f64 }
}

fn evaluate(doc: &GoldDoc, verbose: bool) -> HashMap<String, CatStats> {
    let labels: Vec<String> = if doc.label_set.is_empty() {
        // Default label set when gold doesn't pin one
        vec![
            "PATIENT_NAME", "DOCTOR_NAME", "HN", "LICENSE_NO", "THAI_ID",
            "PHONE", "EMAIL",
        ].into_iter().map(String::from).collect()
    } else {
        doc.label_set.clone()
    };

    let mut per_label: HashMap<String, CatStats> = labels.iter()
        .map(|l| (l.clone(), CatStats::default()))
        .collect();

    for case in &doc.items {
        let predicted: HashSet<&'static str> =
            skuggi_core::scan_categories(&case.raw_text).into_iter().collect();
        let gold: HashSet<String> = case.pii_types_present.iter().cloned().collect();

        if verbose {
            let g: Vec<&str> = labels.iter()
                .filter(|l| gold.contains(*l))
                .map(|l| l.as_str())
                .collect();
            let p: Vec<&str> = predicted.iter().copied().collect();
            let mismatch: Vec<&str> = labels.iter()
                .filter(|l| {
                    let in_gold = gold.contains(*l);
                    let in_pred = gold_to_skuggi(l).iter().any(|s| predicted.contains(s));
                    in_gold != in_pred
                })
                .map(|l| l.as_str())
                .collect();
            let tag = if mismatch.is_empty() { "MATCH".to_string() } else { format!("MISMATCH({:?})", mismatch) };
            println!("  {:18} gold={:?} pred={:?} {}", case.id, g, p, tag);
        }

        for lbl in &labels {
            let in_gold = gold.contains(lbl);
            let in_pred = gold_to_skuggi(lbl).iter().any(|s| predicted.contains(s));
            let stats = per_label.get_mut(lbl).unwrap();
            match (in_gold, in_pred) {
                (true, true) => stats.tp += 1,
                (true, false) => stats.fn_ += 1,
                (false, true) => stats.fp += 1,
                (false, false) => {}
            }
        }
    }
    per_label
}

// ─── Main ────────────────────────────────────────────────────────────────

fn main() -> ExitCode {
    let args = parse_args();
    let path = std::path::Path::new(&args.gold);
    if !path.exists() {
        eprintln!("ERROR: gold file not found: {}", args.gold);
        return ExitCode::from(2);
    }
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => { eprintln!("ERROR: read {}: {}", args.gold, e); return ExitCode::from(2); }
    };
    let doc: GoldDoc = match serde_json::from_slice(&bytes) {
        Ok(d) => d,
        Err(e) => { eprintln!("ERROR: parse {}: {}", args.gold, e); return ExitCode::from(2); }
    };

    println!("Cases: {}  Labels: {:?}", doc.items.len(), doc.label_set);
    println!();

    if args.case_by_case {
        println!("─── Per-case predictions ─────────────────────────────────────");
    }
    let stats = evaluate(&doc, args.case_by_case);
    if args.case_by_case { println!(); }

    println!("─── Per-category metrics ─────────────────────────────────────");
    println!("{:14}  {:>3} {:>3} {:>3}   {:>6} {:>6} {:>6}",
        "category", "TP", "FP", "FN", "prec", "rec", "F1");
    println!("{}", "─".repeat(57));
    let mut below: Vec<String> = Vec::new();
    let labels: Vec<String> = if doc.label_set.is_empty() {
        stats.keys().cloned().collect()
    } else {
        doc.label_set.clone()
    };
    for lbl in &labels {
        let s = stats.get(lbl).unwrap();
        let p = safe_div(s.tp, s.tp + s.fp);
        let r = safe_div(s.tp, s.tp + s.fn_);
        let f1 = if p + r > 0.0 { 2.0 * p * r / (p + r) } else { 0.0 };
        let flag = if f1 >= args.threshold { "" } else { "  ⚠️" };
        println!("{:14}  {:>3} {:>3} {:>3}   {:>6.3} {:>6.3} {:>6.3}{}",
            lbl, s.tp, s.fp, s.fn_, p, r, f1, flag);
        if f1 < args.threshold {
            below.push(lbl.clone());
        }
    }
    println!();
    if !below.is_empty() {
        println!("❌ {} categor{} below F1 ≥ {}: {:?}",
            below.len(),
            if below.len() == 1 { "y" } else { "ies" },
            args.threshold,
            below);
        return ExitCode::from(1);
    }
    println!("✅ All {} categories ≥ F1 {}", labels.len(), args.threshold);
    ExitCode::from(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gold_label_to_skuggi_categories() {
        assert_eq!(gold_to_skuggi("PATIENT_NAME"), vec!["patient_name"]);
        assert_eq!(gold_to_skuggi("THAI_ID"), vec!["thai_id_anchored", "thai_national_id"]);
        assert_eq!(gold_to_skuggi("UNKNOWN"), Vec::<&str>::new());
    }

    #[test]
    fn evaluate_basic_match() {
        let doc = GoldDoc {
            label_set: vec!["PATIENT_NAME".into(), "EMAIL".into()],
            items: vec![
                GoldItem {
                    id: "t1".into(),
                    raw_text: "Patient Name: TESTUSER\nemail test@example.com".into(),
                    pii_types_present: vec!["PATIENT_NAME".into(), "EMAIL".into()],
                },
            ],
        };
        let stats = evaluate(&doc, false);
        assert_eq!(stats["PATIENT_NAME"].tp, 1);
        assert_eq!(stats["EMAIL"].tp, 1);
        assert_eq!(stats["PATIENT_NAME"].fp, 0);
        assert_eq!(stats["PATIENT_NAME"].fn_, 0);
    }

    #[test]
    fn evaluate_negative_control() {
        let doc = GoldDoc {
            label_set: vec!["PATIENT_NAME".into()],
            items: vec![
                GoldItem {
                    id: "neg".into(),
                    raw_text: "The patient is stable. No complications.".into(),
                    pii_types_present: vec![],
                },
            ],
        };
        let stats = evaluate(&doc, false);
        assert_eq!(stats["PATIENT_NAME"].tp, 0);
        assert_eq!(stats["PATIENT_NAME"].fp, 0); // anchor doesn't fire on free text
        assert_eq!(stats["PATIENT_NAME"].fn_, 0);
    }
}
