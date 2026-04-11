import os
from google.cloud import bigquery

def main():
    try:
        project_id = os.environ.get("GOOGLE_CLOUD_PROJECT", "asgard-mimir")
        client = bigquery.Client(project=project_id)
        print(f"✅ Successfully initialized BigQuery Client for project: {client.project}")

        # Test query to check the dimension
        query_dim = """
        SELECT ARRAY_LENGTH(ml_generate_embedding_result) as dim
        FROM `bigquery-public-data.pmc_open_access_commercial.articles`
        WHERE ml_generate_embedding_result IS NOT NULL
        LIMIT 1
        """
        print("🔍 Checking embedding dimension...")
        
        job = client.query(query_dim)
        dim_res = list(job.result())
        dim = dim_res[0].dim if dim_res else 0
        print(f"✅ Google's pre-computed embedding dimension: {dim}")

        # Simulate Method 2: Providing a raw vector manually!
        if dim > 0:
            print("\n🧪 Testing Method 2 (Passing Raw Vector instead of using Vertex AI)...")
            # Create a dummy vector of the right size
            dummy_vector = [0.01] * dim
            vector_str = "[" + ",".join(map(str, dummy_vector)) + "]"
            
            query_vector = f"""
            SELECT base.pmid, distance
            FROM VECTOR_SEARCH(
            TABLE `bigquery-public-data.pmc_open_access_commercial.articles`,
            'ml_generate_embedding_result',
            (SELECT {vector_str} AS vector),
            top_k => 3)
            """
            
            v_job = client.query(query_vector)
            v_res = list(v_job.result())
            
            print("✅ Method 2 Result:")
            for row in v_res:
                print(f"   PMID: {row.pmid:<12} Distance: {row.distance:.4f}")
                
            bytes_processed = v_job.total_bytes_processed
            print(f"\n💰 Total Data Processed: {bytes_processed / (1024**2):.2f} MB")
            
    except Exception as e:
        import traceback
        traceback.print_exc()
        print(f"❌ Error during BigQuery test: {e}")

if __name__ == "__main__":
    main()
