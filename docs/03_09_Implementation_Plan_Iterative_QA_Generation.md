# Enhancing Coverage Score via Iterative Q/A Generation

## Objective
To improve the Pipeline's data extraction quality by using "Missing Facts" identified during the evaluation step (`VERIFY`) as prompts to generate additional Q/A pairs. This process will iteratively improve the Coverage Score of each chunk until it reaches a satisfactory level (close to 100%).

## Methodology: "Iterative Q/A Generation"
The current pipeline extracts Q/A pairs and then evaluates them against the original chunk text to identify "Atomic Facts" and "Missing Facts". 

This extension proposes a mechanism to take the `missing_facts` from the `evaluation_reports` table and feed them back to the LLM. 

### Prompting Strategy
The new generation prompt will provide both the original context and the missing facts:

```text
You are an expert knowledge extractor.
Here is the original source text:
<context>
{chunk_text}
</context>

During previous extraction, the following important facts were missed:
<missing_facts>
{missing_facts_list}
</missing_facts>

Your task is to generate NEW Question/Answer pairs that specifically address and cover the <missing_facts> using only the information provided in the <context>. Do not duplicate existing knowledge. Focus strictly on closing these knowledge gaps.
```

## Proposed Architecture & Implementation Plan

### 1. Backend Data Structures & API (`ro-ai-bridge`)
*   **Database Schema:** The existing `qa_results` and `evaluation_reports` tables are sufficient. New Q/A pairs will simply be appended to `qa_results` with the same `step_id`. 
*   **New Endpoint:** Create `POST /api/pipeline/steps/{id}/generate_missing`.
    *   **Input:** `step_id`
    *   **Logic:**
        1. Fetch `chunk_content` from `pipeline_steps` (joining `document_chunks`).
        2. Fetch `missing_facts` from `evaluation_reports`.
        3. If `missing_facts` is empty, return early.
        4. Construct the prompt with the chunk and missing facts.
        5. Call the LLM provider to generate new Q/A pairs.
        6. Parse the LLM response and insert new records into `qa_results`.
        7. Trigger the `VERIFY` step logic again for this `step_id` to re-evaluate the Coverage Score with the newly added Q/A pairs.
        8. Return the updated coverage score and new Q/A pairs.

### 2. Frontend UI (`ro-ai-dashboard`)
*   **Evaluation Report Component:** 
    *   Modify the `Step Details` page (`src/app/runs/[id]/steps/[step_id]/page.tsx` or wherever the Step detail dialog is implemented).
    *   In the "Evaluation Report" section, if the `coverage_score` is less than 1.0 (or a configurable threshold like 0.95) and `missing_facts` exist, display a button: **"✨ Generate Missing Q/A"**.
*   **Action Flow:**
    *   Clicking the button calls the new backend endpoint.
    *   Show a loading state (spinner).
    *   Upon success, refresh the step details to display the newly added Q/A pairs and the updated (higher) Coverage Score and updated missing facts list.

## Future Considerations
*   **Automated Iteration:** Instead of requiring a manual button click, this could be built into the `FULL_PROCESS` pipeline logic itself. For example: `While coverage_score < 0.9 and iterations < 3: generate_missing_qa()`.
*   **Cost Management:** Iterative generation uses more API tokens. This should be a configurable option (e.g., "Auto-improve extraction quality").
