---
description: enforce Agile Scrum feature management and sprint scope discipline
---

# Agile Feature Management Rules

As an AI Assistant on Project Mimir, you must enforce discipline around Sprint Scopes and Feature Management, balancing Agile adaptability with ISO 29110 traceability.

1. **The Product Backlog**: When the user asks for a new feature, a change in requirements, or reports a non-critical bug that is NOT part of the current active sprint, you MUST NOT implement it immediately. Instead, create a GitHub Issue to log it into the Product Backlog.
2. **Sprint Scope Sanctity**: The active sprint scope is defined in the current `docs/03_*_Implementation_Plan_*.md`. You should strongly advise the user against adding new features mid-sprint. 
3. **Mid-Sprint Adjustments**: If the user insists on adding a feature mid-sprint, you MUST ask the user which existing feature from the current sprint should be removed to accommodate the new work (Trade-off). If agreed, update the Implementation Plan and the `PM_02_Status_Reports.md` Change Logs.
4. **Sprint Planning**: Between sprints, assist the user in reviewing the Product Backlog (Open Issues), prioritizing them, and defining the scope for the next sprint in a new or updated Implementation Plan document.
