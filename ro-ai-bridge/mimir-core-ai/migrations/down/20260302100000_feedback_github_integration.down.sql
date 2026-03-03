-- Rollback: Sprint 14 Phase 3 — Feedback GitHub Integration

DROP INDEX idx_feedback_github_issue ON feedback_reports;

ALTER TABLE feedback_reports
    DROP COLUMN client_logs,
    DROP COLUMN system_logs,
    DROP COLUMN github_issue_number,
    DROP COLUMN github_issue_url;
