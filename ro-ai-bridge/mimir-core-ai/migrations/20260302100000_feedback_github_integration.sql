-- Sprint 14 Phase 3: Feedback → GitHub Issue Integration
-- Adds GitHub issue tracking and log capture columns to feedback_reports

ALTER TABLE feedback_reports
    ADD COLUMN github_issue_url VARCHAR(500) NULL AFTER resolution,
    ADD COLUMN github_issue_number INT NULL AFTER github_issue_url,
    ADD COLUMN system_logs TEXT NULL AFTER github_issue_number,
    ADD COLUMN client_logs TEXT NULL AFTER system_logs;

-- Index for GitHub issue lookup
CREATE INDEX idx_feedback_github_issue ON feedback_reports(github_issue_number);
