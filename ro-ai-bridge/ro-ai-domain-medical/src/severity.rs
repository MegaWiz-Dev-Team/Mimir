//! DDI severity gate — assigns a clinical severity to a drug-drug finding.
//!
//! WHY: PrimeKG `DRUG_DRUG` has no severity (2.67M flat edges), so the naive
//! pruner over-prunes (~66% on managed combos). DDInter has severity but is
//! CC BY-NC-SA (non-commercial) → cannot ship. So the PRODUCT gate uses a
//! curated, **license-clean** rules table (ONC-15 high-priority DDIs, FDA-label
//! contraindications, standard references) compiled in via `include_str!`.
//!
//! Keyed on PrimeKG-canonical (ingredient) names, lowercased, order-independent.
//! An unlisted pair defaults to **Moderate** — "an interaction exists, severity
//! not established". We NEVER downgrade to safe without evidence; only the
//! explicit managed allowlist earns `Minor`.
//!
//! DDInter is used ONLY as a firewalled eval benchmark to measure over-prune
//! reduction — never shipped.

use std::collections::HashMap;

const RULES_TSV: &str = include_str!("../data/ddi_severity_rules.tsv");

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Minor,           // managed / routinely co-prescribed
    Moderate,        // interaction exists, severity not established (default)
    Major,           // clinically significant
    Contraindicated, // do not co-administer
}

impl Severity {
    pub fn parse(s: &str) -> Option<Severity> {
        match s.trim().to_ascii_lowercase().as_str() {
            "minor" => Some(Severity::Minor),
            "moderate" => Some(Severity::Moderate),
            "major" => Some(Severity::Major),
            "contraindicated" => Some(Severity::Contraindicated),
            _ => None,
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Severity::Minor => "minor",
            Severity::Moderate => "moderate",
            Severity::Major => "major",
            Severity::Contraindicated => "contraindicated",
        }
    }
}

/// Order-independent lowercased pair key.
fn key(a: &str, b: &str) -> (String, String) {
    let (a, b) = (a.trim().to_ascii_lowercase(), b.trim().to_ascii_lowercase());
    if a <= b { (a, b) } else { (b, a) }
}

pub struct SeverityGate {
    rules: HashMap<(String, String), Severity>,
}

impl SeverityGate {
    pub fn load() -> Self {
        let mut rules = HashMap::new();
        for line in RULES_TSV.lines() {
            let l = line.trim();
            if l.is_empty() || l.starts_with('#') {
                continue;
            }
            let f: Vec<&str> = l.split('\t').collect();
            if f.len() >= 3 {
                if let Some(sev) = Severity::parse(f[2]) {
                    rules.insert(key(f[0], f[1]), sev);
                }
            }
        }
        Self { rules }
    }

    /// Severity for a drug-drug pair (PrimeKG-canonical names). Unlisted → Moderate.
    pub fn drug_drug(&self, a: &str, b: &str) -> Severity {
        self.rules.get(&key(a, b)).copied().unwrap_or(Severity::Moderate)
    }

    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

impl Default for SeverityGate {
    fn default() -> Self {
        Self::load()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_load_and_lookup() {
        let g = SeverityGate::load();
        assert!(g.rule_count() >= 10, "too few rules: {}", g.rule_count());
        // order-independent + case-insensitive
        assert_eq!(g.drug_drug("Warfarin", "Acetylsalicylic acid"), Severity::Major);
        assert_eq!(g.drug_drug("Acetylsalicylic acid", "warfarin"), Severity::Major);
        assert_eq!(g.drug_drug("sildenafil", "nitroglycerin"), Severity::Contraindicated);
        assert_eq!(g.drug_drug("warfarin", "acetaminophen"), Severity::Minor);
    }

    #[test]
    fn unlisted_defaults_moderate_never_safe() {
        let g = SeverityGate::load();
        assert_eq!(g.drug_drug("amoxicillin", "loratadine"), Severity::Moderate);
    }

    #[test]
    fn severity_is_ordered() {
        assert!(Severity::Contraindicated > Severity::Major);
        assert!(Severity::Major > Severity::Moderate);
        assert!(Severity::Moderate > Severity::Minor);
    }
}
