//! Character / Word Error Rate + summary statistics.
//!
//! Pure-Rust replacement for `Syn/benchmarks/metrics.py` so OCR benchmarks
//! (B-50h.0) and future Eir agent evals can share the same scoring
//! primitives without re-implementing Levenshtein per language.
//!
//! Standard library only — no third-party deps.

/// Wagner-Fischer DP edit distance over arbitrary token lists. Operates
/// on slices so the same function backs both character-level (`cer`) and
/// word-level (`wer`) scoring.
fn levenshtein<T: PartialEq>(a: &[T], b: &[T]) -> usize {
    if a.is_empty() { return b.len(); }
    if b.is_empty() { return a.len(); }
    let n = b.len();
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr: Vec<usize> = vec![0; n + 1];
    for (i, ca) in a.iter().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr[j + 1] = (curr[j] + 1) // insert
                .min(prev[j + 1] + 1)   // delete
                .min(prev[j] + cost);   // substitute
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Strip leading/trailing whitespace and collapse internal whitespace
/// runs to a single space. Case and punctuation are preserved — those
/// are real OCR errors when they happen.
fn normalize(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Character Error Rate (CER). Values in [0, ~1.0+] — values >1 are
/// possible when the hypothesis is much longer than the reference.
///
/// Returns 0.0 when both inputs are empty, 1.0 when the reference is
/// empty but the hypothesis isn't.
pub fn cer(reference: &str, hypothesis: &str) -> f64 {
    let r = normalize(reference);
    let h = normalize(hypothesis);
    if r.is_empty() {
        return if h.is_empty() { 0.0 } else { 1.0 };
    }
    let r_chars: Vec<char> = r.chars().collect();
    let h_chars: Vec<char> = h.chars().collect();
    let edits = levenshtein(&r_chars, &h_chars);
    edits as f64 / r_chars.len() as f64
}

/// Word Error Rate (WER). Same shape as CER, tokenised on whitespace.
pub fn wer(reference: &str, hypothesis: &str) -> f64 {
    let r = normalize(reference);
    let h = normalize(hypothesis);
    let r_words: Vec<&str> = r.split(' ').filter(|w| !w.is_empty()).collect();
    let h_words: Vec<&str> = h.split(' ').filter(|w| !w.is_empty()).collect();
    if r_words.is_empty() {
        return if h_words.is_empty() { 0.0 } else { 1.0 };
    }
    let edits = levenshtein(&r_words, &h_words);
    edits as f64 / r_words.len() as f64
}

/// Summary statistics for a slice of numeric values: min / mean /
/// median / p95 / max / count. `None`-valued entries are dropped.
#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    pub count: usize,
    pub min: Option<f64>,
    pub mean: Option<f64>,
    pub median: Option<f64>,
    pub p95: Option<f64>,
    pub max: Option<f64>,
}

impl Summary {
    pub fn empty() -> Self {
        Self { count: 0, min: None, mean: None, median: None, p95: None, max: None }
    }
}

/// Summarise an iterator of optional values. Same shape as the Python
/// helper in `Syn/benchmarks/metrics.py`.
pub fn summarize<I: IntoIterator<Item = Option<f64>>>(values: I) -> Summary {
    let mut vals: Vec<f64> = values.into_iter().flatten().collect();
    if vals.is_empty() {
        return Summary::empty();
    }
    vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = vals.len();
    let sum: f64 = vals.iter().sum();
    Summary {
        count: n,
        min: Some(vals[0]),
        mean: Some(sum / n as f64),
        median: Some(vals[n / 2]),
        p95: Some(vals[(((n as f64) * 0.95) as usize).saturating_sub(1).min(n - 1)]),
        max: Some(vals[n - 1]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cer_identical_is_zero() {
        assert_eq!(cer("hello world", "hello world"), 0.0);
    }

    #[test]
    fn cer_one_substitution() {
        // "hello" → "h3llo" = 1 edit / 5 chars (normalized = "hello world" / "h3llo w0rld")
        // 2 edits / 11 chars normalized = 0.1818...
        let v = cer("hello world", "h3llo w0rld");
        assert!((v - 2.0 / 11.0).abs() < 1e-9);
    }

    #[test]
    fn cer_handles_empty_reference() {
        assert_eq!(cer("", ""), 0.0);
        assert_eq!(cer("", "noise"), 1.0);
    }

    #[test]
    fn wer_one_word_changed() {
        let v = wer("hello world", "hello WORLD");
        assert!((v - 0.5).abs() < 1e-9);
    }

    #[test]
    fn wer_identical_is_zero() {
        assert_eq!(wer("alpha beta gamma", "alpha beta gamma"), 0.0);
    }

    #[test]
    fn normalize_collapses_whitespace() {
        assert_eq!(normalize("  a   b\tc\n d  "), "a b c d");
    }

    #[test]
    fn summarize_empty_returns_empty() {
        let s = summarize(std::iter::empty());
        assert_eq!(s.count, 0);
        assert!(s.mean.is_none());
    }

    #[test]
    fn summarize_basic_distribution() {
        // values 1..=10; median=6, mean=5.5, p95=9 (index ceil(0.95*10)-1=9 → 10),
        // we use saturating_sub so p95 lands at index 9 = 10.
        let s = summarize((1..=10).map(|n| Some(n as f64)));
        assert_eq!(s.count, 10);
        assert_eq!(s.min, Some(1.0));
        assert_eq!(s.max, Some(10.0));
        assert!((s.mean.unwrap() - 5.5).abs() < 1e-9);
    }

    #[test]
    fn summarize_drops_none_entries() {
        let s = summarize(vec![Some(1.0), None, Some(3.0)]);
        assert_eq!(s.count, 2);
        assert_eq!(s.min, Some(1.0));
        assert_eq!(s.max, Some(3.0));
    }
}
