use anyhow::{Context, Result};
use mimir_core_ai::services::chunking::{chunk, ChunkResult, ChunkStrategy};
use crate::bigquery::{PubMedBigQueryService, FetchedArticle};
use crate::curation::{categorize_article, ArticleTier};

#[derive(Debug)]
pub struct ProcessedArticle {
    pub pmid: String,
    pub title: String,
    pub tier: ArticleTier,
    pub chunks: Vec<ChunkResult>,
}

/// Client for orchestrating PubMed caching from BigQuery to Qdrant Context.
pub struct PubmedCacheLoader<'a> {
    bq_service: &'a PubMedBigQueryService,
}

impl<'a> PubmedCacheLoader<'a> {
    pub fn new(bq_service: &'a PubMedBigQueryService) -> Self {
        Self { bq_service }
    }

    /// Fetch articles matching clinical criteria, categorize, and chunk them.
    pub async fn load_to_context(&self, clinical_query: &str, limit: u32) -> Result<Vec<ProcessedArticle>> {
        let bq_articles = self.bq_service.fetch_public_articles(clinical_query, limit)
            .await
            .context("Failed fetching articles from BigQuery Public Dataset")?;

        let mut processed = Vec::new();

        for article in bq_articles {
            let tier = categorize_article(&article.article_text);
            let combined_text = format!("{}\n\n{}", article.title, article.article_text);
            let chunks = chunk(&combined_text, &ChunkStrategy::Recursive { max_size: 500 })
                .unwrap_or_default();

            processed.push(ProcessedArticle {
                pmid: article.pmid,
                title: article.title,
                tier,
                chunks,
            });
        }

        Ok(processed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gcp_bigquery_client::Client;

    // Unit testing here usually involves mocking the BQ service, but since
    // PubMedBigQueryService fetches real data, we just ensure the struct signature compiles.
    #[test]
    fn test_processed_article_struct() {
        let p = ProcessedArticle {
            pmid: "123".to_string(),
            title: "Test".to_string(),
            tier: ArticleTier::Evidence,
            chunks: vec![],
        };
        assert_eq!(p.pmid, "123");
    }
}
