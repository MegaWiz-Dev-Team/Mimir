---
description: enforce ISO/IEC 29110 documentation updates
---
# ISO/IEC 29110 Compliance Rules

As an AI Assistant on Project Mimir, you must ensure that all significant progress, testing, and project management activities are properly documented according to ISO/IEC 29110 standards.

1. **Sprint Planning & Status (PM-02)**: When a Sprint begins or ends, you must update or create the relevant `docs/iso_29110/pm/` documents (e.g., `PM_02_Status_Reports.md`, Sprint Completion Reports).
2. **Issue Logging (PM-02)**: ALWAYS append new bugs or architectural changes to the Issue / Change Logs table in `PM_02_Status_Reports.md`.
3. **Test Scripts (SI-04)**: When you perform manual testing or verify a bug fix, you must update the corresponding `docs/iso_29110/si/SI_04_*_TestScript.md`. Ensure you mark items as `✅ Pass` or `❌ Fail` and provide clear notes or image references.
4. **Implementation Plans (SI-02)**: Do not stray from the Phase/Sprint plans defined in `docs/03_*_Implementation_Plan_*.md` without explicit permission from the user. If an architectural decision needs changing, propose the change in the documents first.
