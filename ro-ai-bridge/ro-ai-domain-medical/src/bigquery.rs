use anyhow::{Context, Result};
use gcp_bigquery_client::Client;
use gcp_bigquery_client::model::query_request::QueryRequest;

#[derive(Debug)]
pub struct FetchedArticle {
    pub pmid: String,
    pub title: String,
    pub article_text: String,
}

/// Service for interacting with Google BigQuery to query PubMed data.
pub struct PubMedBigQueryService {
    client: Client,
    project_id: String,
}

impl PubMedBigQueryService {
    /// Initialize the BigQuery service. ⛔ Refuses to construct unless the
    /// caller explicitly opts in via `ALLOW_BIGQUERY=1` env var.
    ///
    /// Past usage cost ~$67 USD; prefer NCBI E-utilities or PMC FTP bulk
    /// (both free) for routine PubMed ingestion.
    pub async fn new(project_id: &str) -> Result<Self> {
        if std::env::var("ALLOW_BIGQUERY").unwrap_or_default() != "1" {
            return Err(anyhow::anyhow!(
                "BigQuery PubMed ingestion is disabled to prevent cost overruns. \
                 Set ALLOW_BIGQUERY=1 to override (consult cost owner first). \
                 Free alternatives: scripts/sync_pubmed_incremental.py (NCBI E-utilities), \
                 scripts/sync_pubmed_pmc_bulk.py (PMC FTP)."
            ));
        }
        let client = Client::from_application_default_credentials()
            .await
            .context("Failed to initialize GCP BigQuery Client from environment")?;

        Ok(Self {
            client,
            project_id: project_id.to_string(),
        })
    }

    pub async fn fetch_public_articles(&self, query_text: &str, limit: u32) -> Result<Vec<FetchedArticle>> {
        // Implement Plan A: BigQuery Full-text Search (100% Free Tier).
        // Since Google's dataset embeddings require the proprietary `textembedding-gecko` model
        // to query, we perform a standard keyword search to filter the 2.3M articles down to a manageable size.
        // The Mimir auto-pipeline will then natively chunk and semantically embed the results locally using Heimdall!
        
        // Basic sanitization for the query string to prevent simple SQL injection
        let safe_query = query_text.replace("'", "\\'");
        
        let sql = format!(
            r#"SELECT pmid, title, article_text 
               FROM `bigquery-public-data.pmc_open_access_commercial.articles`
               WHERE LOWER(title) LIKE LOWER('%{query}%') 
                  OR LOWER(article_text) LIKE LOWER('%{query}%')
               LIMIT {limit}"#,
            query = safe_query,
            limit = limit
        );

        let req = QueryRequest::new(sql);
        let mut rs = self.client.job().query(&self.project_id, req)
            .await
            .context("Failed executing VECTOR_SEARCH on BigQuery public dataset")?;

        let mut articles = Vec::new();

        while rs.next_row() {
            let pmid = rs.get_string(0).unwrap_or_default().unwrap_or_default();
            let title = rs.get_string(1).unwrap_or_default().unwrap_or_default();
            let article_text = rs.get_string(2).unwrap_or_default().unwrap_or_default();

            articles.push(FetchedArticle { pmid, title, article_text });
        }

        Ok(articles)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Since we don't have GCP credentials during unit tests, we primarily test
    // that the Client initialization expects GOOGLE_APPLICATION_CREDENTIALS.
    #[tokio::test]
    async fn test_new_client_initialization_does_not_panic() {
        // Just verify it doesn't panic on instantiation.
        // It might fail gracefully (Err) or succeed (Ok) based on local ADC.
        let res = PubMedBigQueryService::new("my-proj").await;
        if let Err(e) = res {
            assert!(e.to_string().contains("Failed to initialize GCP BigQuery Client"));
        }
    }
}
