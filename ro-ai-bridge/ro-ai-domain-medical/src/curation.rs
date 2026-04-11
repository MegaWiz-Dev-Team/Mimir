#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ArticleTier {
    Guidelines,
    Evidence,
    Context,
}

impl ArticleTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            ArticleTier::Guidelines => "Guidelines",
            ArticleTier::Evidence => "Evidence",
            ArticleTier::Context => "Context",
        }
    }
}

/// Categorize article based on keyword presence in text.
///
/// Rules:
/// - Guidelines: contains "guideline", "recommendation", "consensus", "cpg"
/// - Evidence: contains "trial", "study", "cohort", "evidence", "results", "meta-analysis"
/// - Context: default fallback
pub fn categorize_article(text: &str) -> ArticleTier {
    let lower_text = text.to_lowercase();
    
    // Check for guidelines
    if lower_text.contains("guideline") ||
       lower_text.contains("recommendation") ||
       lower_text.contains("consensus") ||
       lower_text.contains("cpg") {
        return ArticleTier::Guidelines;
    }
    
    // Check for evidence
    if lower_text.contains("trial") ||
       lower_text.contains("study") ||
       lower_text.contains("cohort") ||
       lower_text.contains("evidence") ||
       lower_text.contains("results") ||
       lower_text.contains("meta-analysis") {
        return ArticleTier::Evidence;
    }
    
    // Fallback
    ArticleTier::Context
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_categorize_guidelines() {
        assert_eq!(categorize_article("This is a clinical consensus."), ArticleTier::Guidelines);
        assert_eq!(categorize_article("Practice Guideline for Asthma"), ArticleTier::Guidelines);
        assert_eq!(categorize_article("Recommendation: Do not use X"), ArticleTier::Guidelines);
    }

    #[test]
    fn test_categorize_evidence() {
        assert_eq!(categorize_article("A randomized controlled trial of Y"), ArticleTier::Evidence);
        assert_eq!(categorize_article("Our study shows that..."), ArticleTier::Evidence);
        assert_eq!(categorize_article("Cohort patient demographics"), ArticleTier::Evidence);
    }

    #[test]
    fn test_categorize_context() {
        assert_eq!(categorize_article("Hypertension is a tricky disease. We must be careful."), ArticleTier::Context);
        assert_eq!(categorize_article("General background on the history of medicine."), ArticleTier::Context);
    }
}
