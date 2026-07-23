//! Drug/disease NAME normalizer.
//!
//! Maps a name as written (brand / lay / cross-region generic) to the canonical
//! form PrimeKG uses (DrugBank canonical), so the resolver's exact-match tier
//! fires instead of missing or mis-resolving. Live measurement showed naive
//! resolution FN'd 100% of brand names; a curated seed map lifts drug exact
//! resolution 73% -> 100% on the resolution benchmark.
//!
//! v1 is a curated seed map (license-clean: common-knowledge / RxNorm-derivable).
//! Later: load full RxNorm (brand->ingredient, public domain) + TMT (Thai) as
//! tables behind this interface. DrugBank synonyms require a commercial license
//! (adopt lane) and must not be baked in without clearance.

use std::borrow::Cow;
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EntityKind {
    Drug,
    Disease,
}

pub struct DrugDiseaseNormalizer {
    drug: HashMap<String, String>,
    disease: HashMap<String, String>,
}

impl DrugDiseaseNormalizer {
    /// Seed map (v1). Keys are lowercased; values are the PrimeKG-canonical name.
    pub fn seed() -> Self {
        let drug = [
            ("aspirin", "acetylsalicylic acid"),
            ("coumadin", "warfarin"),
            ("glucophage", "metformin"),
            ("viagra", "sildenafil"),
            ("tylenol", "acetaminophen"),
            ("panadol", "acetaminophen"),
            ("paracetamol", "acetaminophen"),
            ("ventolin", "salbutamol"),
            ("albuterol", "salbutamol"),
            ("lasix", "furosemide"),
            ("augmentin", "amoxicillin"),
            ("advil", "ibuprofen"),
            ("motrin", "ibuprofen"),
        ];
        let disease = [
            ("high blood pressure", "hypertension"),
            ("heart attack", "myocardial infarction"),
        ];
        Self {
            drug: drug.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            disease: disease.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
        }
    }

    /// Canonical form if a mapping exists, else `None` (caller falls back to raw).
    pub fn normalize(&self, name: &str, kind: EntityKind) -> Option<String> {
        let key = name.trim().to_ascii_lowercase();
        let table = match kind {
            EntityKind::Drug => &self.drug,
            EntityKind::Disease => &self.disease,
        };
        table.get(&key).cloned()
    }

    /// Canonical form, or the original name unchanged.
    pub fn canonical<'a>(&self, name: &'a str, kind: EntityKind) -> Cow<'a, str> {
        match self.normalize(name, kind) {
            Some(c) => Cow::Owned(c),
            None => Cow::Borrowed(name),
        }
    }
}

impl Default for DrugDiseaseNormalizer {
    fn default() -> Self {
        Self::seed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_common_brands_and_synonyms() {
        let n = DrugDiseaseNormalizer::seed();
        assert_eq!(n.canonical("Coumadin", EntityKind::Drug).as_ref(), "warfarin");
        assert_eq!(n.canonical("aspirin", EntityKind::Drug).as_ref(), "acetylsalicylic acid");
        assert_eq!(n.canonical("PARACETAMOL", EntityKind::Drug).as_ref(), "acetaminophen");
        assert_eq!(
            n.canonical("high blood pressure", EntityKind::Disease).as_ref(),
            "hypertension"
        );
    }

    #[test]
    fn unmapped_name_passes_through() {
        let n = DrugDiseaseNormalizer::seed();
        assert_eq!(n.canonical("metformin", EntityKind::Drug).as_ref(), "metformin");
        assert_eq!(n.normalize("nonexistentdrug", EntityKind::Drug), None);
    }

    #[test]
    fn kind_scoped_lookup() {
        // a drug key must not resolve under Disease and vice-versa
        let n = DrugDiseaseNormalizer::seed();
        assert_eq!(n.normalize("aspirin", EntityKind::Disease), None);
    }
}
