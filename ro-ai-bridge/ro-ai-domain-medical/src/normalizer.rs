//! Drug/disease NAME normalizer.
//!
//! Maps a name as written (brand / lay / cross-region generic) to the canonical
//! form PrimeKG uses (DrugBank canonical), so the resolver's exact-match tier
//! fires instead of missing or mis-resolving. Live measurement showed naive
//! resolution FN'd 100% of brand names; this normalizer lifts drug exact
//! resolution 47.8% -> 95.7% on a 69-drug probe (24 brands beyond the seed).
//!
//! Pipeline (see docs/NORMALIZER.md):
//!   drug:    TMT thai-trade->generic  ->  RxNorm brand->ingredient  ->  US<->INN alias  ->  PrimeKG resolve
//!   disease: lay->canonical override    ->  PrimeKG resolve
//!
//! Tables are **compiled in via include_str!** and built dev-time — no runtime
//! file or network access (Asgard is offline-first). Sources:
//!   - RxNorm (public domain, RxNav) — brand->ingredient. Ships.
//!   - TMT (THIS-Center/MoPH, free in TH) — Thai trade->generic (Lane B); the Thai
//!     names the US/EN RxNorm table can't see. Built by scripts/build_tmt_table.py.
//!   - hand-curated US<->INN alias + disease overrides — license-clean.
//! DDInter (CC BY-NC-SA) and DrugBank (commercial) are NOT used here — see the doc.

use std::borrow::Cow;
use std::collections::HashMap;

const RXNORM_TSV: &str = include_str!("../data/rxnorm_brand_ingredient.tsv");
const ALIAS_TSV: &str = include_str!("../data/us_inn_alias.tsv");
const DISEASE_TSV: &str = include_str!("../data/disease_overrides.tsv");
const TMT_TSV: &str = include_str!("../data/tmt_thai_generic.tsv");

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EntityKind {
    Drug,
    Disease,
}

pub struct DrugDiseaseNormalizer {
    thai_to_generic: HashMap<String, String>,     // TMT: Thai trade -> generic (Lane B)
    brand_to_ingredient: HashMap<String, String>, // RxNorm: brand -> ingredient
    alias: HashMap<String, String>,               // US/common -> PrimeKG canonical (INN)
    disease: HashMap<String, String>,             // lay term -> canonical disease
}

/// Parse a two-column `key<TAB>value` table; `#` comments and blanks skipped.
/// Keys and values are lowercased (resolve is case-insensitive downstream). For a `;`-separated
/// combo value (e.g. `amoxicillin;clavulanate`), the loader takes the FIRST ingredient — the full
/// brand→ingredient table orders it so the PrimeKG-preferred name is first.
fn parse_tsv(text: &str) -> HashMap<String, String> {
    let mut m = HashMap::new();
    for line in text.lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let mut it = l.split('\t');
        if let (Some(k), Some(v)) = (it.next(), it.next()) {
            let k = k.trim();
            let v = v.split(';').next().unwrap_or("").trim();
            if !k.is_empty() && !v.is_empty() {
                m.insert(k.to_ascii_lowercase(), v.to_ascii_lowercase());
            }
        }
    }
    m
}

impl DrugDiseaseNormalizer {
    /// Load the shipped static tables (compiled in). No runtime I/O.
    pub fn load() -> Self {
        Self {
            thai_to_generic: parse_tsv(TMT_TSV),
            brand_to_ingredient: parse_tsv(RXNORM_TSV),
            alias: parse_tsv(ALIAS_TSV),
            disease: parse_tsv(DISEASE_TSV),
        }
    }

    /// Back-compat alias for `load()` — kept because `PrimeKgPruner::new` calls it.
    pub fn seed() -> Self {
        Self::load()
    }

    /// Canonical form if any layer maps the name, else `None` (caller falls back
    /// to the raw term, which may already be canonical, e.g. "metformin").
    pub fn normalize(&self, name: &str, kind: EntityKind) -> Option<String> {
        let k = name.trim().to_ascii_lowercase();
        match kind {
            EntityKind::Drug => {
                // layer 0: Thai trade -> generic (TMT). The Thai lane the US/EN RxNorm
                // table can't see; its output feeds the layers below.
                let thai = self.thai_to_generic.get(&k);
                let start = thai.cloned().unwrap_or_else(|| k.clone());
                // layer 1: brand -> ingredient (RxNorm)
                let mapped_brand = self.brand_to_ingredient.get(&start);
                let base = mapped_brand.cloned().unwrap_or(start);
                // layer 2: US/common generic -> PrimeKG canonical (INN)
                let aliased = self.alias.get(&base).cloned();
                match (thai.is_some(), mapped_brand.is_some(), aliased.is_some()) {
                    (false, false, false) => None, // nothing mapped — pass the original through
                    _ => Some(aliased.unwrap_or(base)),
                }
            }
            EntityKind::Disease => self.disease.get(&k).cloned(),
        }
    }

    /// Canonical form, or the original name unchanged.
    pub fn canonical<'a>(&self, name: &'a str, kind: EntityKind) -> Cow<'a, str> {
        match self.normalize(name, kind) {
            Some(c) => Cow::Owned(c),
            None => Cow::Borrowed(name),
        }
    }

    /// (rxnorm, alias, disease, tmt) table sizes — for startup logging / sanity checks.
    pub fn table_sizes(&self) -> (usize, usize, usize, usize) {
        (
            self.brand_to_ingredient.len(),
            self.alias.len(),
            self.disease.len(),
            self.thai_to_generic.len(),
        )
    }
}

impl Default for DrugDiseaseNormalizer {
    fn default() -> Self {
        Self::load()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_load_nonempty() {
        let n = DrugDiseaseNormalizer::load();
        let (rx, al, di, tmt) = n.table_sizes();
        assert!(rx >= 50, "rxnorm table too small: {rx}");
        assert!(al >= 2 && di >= 2, "alias/disease tables missing");
        assert!(tmt >= 1000, "TMT thai->generic table too small: {tmt}");
    }

    #[test]
    fn full_table_brand_and_combo() {
        let n = DrugDiseaseNormalizer::load();
        // full RxNorm table, ordered to PrimeKG's preferred name, then the generalized alias maps
        // the INN synonym to the exact PrimeKG node name (paracetamol → acetaminophen).
        assert_eq!(n.canonical("tylenol", EntityKind::Drug).as_ref(), "acetaminophen");
        // the alias generalization works on a bare INN generic too, not just via a brand.
        assert_eq!(n.canonical("paracetamol", EntityKind::Drug).as_ref(), "acetaminophen");
    }

    #[test]
    fn thai_trade_via_tmt() {
        let n = DrugDiseaseNormalizer::load();
        // Lane B: Thai brand -> generic (TMT) -> alias -> PrimeKG name. sara -> paracetamol (TMT)
        // -> acetaminophen (alias), the full end-to-end canonical the pruner resolves against.
        assert_eq!(n.canonical("Sara", EntityKind::Drug).as_ref(), "acetaminophen");
        assert_eq!(n.canonical("brufen", EntityKind::Drug).as_ref(), "ibuprofen");
        assert_eq!(n.canonical("ponstan", EntityKind::Drug).as_ref(), "mefenamic acid");
    }

    #[test]
    fn brand_to_ingredient_via_rxnorm() {
        let n = DrugDiseaseNormalizer::load();
        assert_eq!(n.canonical("Coumadin", EntityKind::Drug).as_ref(), "warfarin");
        assert_eq!(n.canonical("lipitor", EntityKind::Drug).as_ref(), "atorvastatin");
        assert_eq!(n.canonical("XANAX", EntityKind::Drug).as_ref(), "alprazolam");
    }

    #[test]
    fn us_inn_alias_applied() {
        let n = DrugDiseaseNormalizer::load();
        // aspirin is not a brand; the alias maps common -> INN canonical
        assert_eq!(
            n.canonical("aspirin", EntityKind::Drug).as_ref(),
            "acetylsalicylic acid"
        );
        // ventolin -> albuterol (RxNorm) -> salbutamol (alias, PrimeKG's INN node)
        assert_eq!(n.canonical("ventolin", EntityKind::Drug).as_ref(), "salbutamol");
    }

    #[test]
    fn unmapped_passes_through_and_disease() {
        let n = DrugDiseaseNormalizer::load();
        assert_eq!(n.normalize("metformin", EntityKind::Drug), None); // already canonical
        assert_eq!(
            n.canonical("high blood pressure", EntityKind::Disease).as_ref(),
            "hypertension"
        );
        assert_eq!(n.normalize("aspirin", EntityKind::Disease), None); // kind-scoped
    }
}
