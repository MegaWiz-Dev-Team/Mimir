//! Backup & Disaster Recovery Service (Issue #158)
//!
//! Backup status API, path generation, retention management.
//! Supports MariaDB, Qdrant, and configuration backups.

use chrono::{Datelike, NaiveDate, Utc};
use serde::Serialize;

// ═══════════════════════════════════════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════════════════════════════════════

/// Backup type discriminator
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum BackupType {
    MariaDB,
    Qdrant,
    Config,
    Full,
}

impl BackupType {
    pub fn as_str(&self) -> &str {
        match self {
            BackupType::MariaDB => "mariadb",
            BackupType::Qdrant => "qdrant",
            BackupType::Config => "config",
            BackupType::Full => "full",
        }
    }
}

/// Backup configuration
#[derive(Debug, Clone)]
pub struct BackupConfig {
    /// Base directory for backups
    pub backup_dir: String,
    /// Number of daily backups to keep
    pub daily_retention: usize,
    /// Number of weekly backups to keep
    pub weekly_retention: usize,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            backup_dir: "/data/backups".to_string(),
            daily_retention: 7,
            weekly_retention: 4,
        }
    }
}

/// A single backup entry
#[derive(Debug, Clone, Serialize)]
pub struct BackupEntry {
    pub filename: String,
    pub backup_type: BackupType,
    pub date: String,
    pub size_bytes: Option<u64>,
    pub path: String,
}

/// Backup status response
#[derive(Debug, Serialize)]
pub struct BackupStatus {
    pub enabled: bool,
    pub backup_dir: String,
    pub total_backups: usize,
    pub latest_backup: Option<BackupEntry>,
    pub daily_count: usize,
    pub weekly_count: usize,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Pure Functions — TDD-testable (no I/O)
// ═══════════════════════════════════════════════════════════════════════════════

/// Generate a backup file path with timestamp.
///
/// Format: `{backup_dir}/{type}/mimir_{type}_{YYYYMMDD_HHMMSS}.{ext}`
pub fn generate_backup_path(config: &BackupConfig, backup_type: &BackupType) -> String {
    let now = Utc::now().format("%Y%m%d_%H%M%S");
    let type_str = backup_type.as_str();
    let ext = match backup_type {
        BackupType::MariaDB => "sql.gz",
        BackupType::Qdrant => "snapshot",
        BackupType::Config => "tar.gz",
        BackupType::Full => "tar.gz",
    };
    format!(
        "{}/{}/mimir_{}_{}.{}",
        config.backup_dir, type_str, type_str, now, ext
    )
}

/// Parse a backup filename into a BackupEntry.
///
/// Expected format: `mimir_{type}_{YYYYMMDD_HHMMSS}.{ext}`
pub fn parse_backup_filename(filename: &str, base_dir: &str) -> Option<BackupEntry> {
    // Strip extension(s) for parsing
    let name_without_ext = filename
        .strip_suffix(".sql.gz")
        .or_else(|| filename.strip_suffix(".snapshot"))
        .or_else(|| filename.strip_suffix(".tar.gz"))?;

    // Expected: mimir_{type}_{date}_{time}
    let parts: Vec<&str> = name_without_ext.splitn(4, '_').collect();
    if parts.len() < 4 || parts[0] != "mimir" {
        return None;
    }

    let backup_type = match parts[1] {
        "mariadb" => BackupType::MariaDB,
        "qdrant" => BackupType::Qdrant,
        "config" => BackupType::Config,
        "full" => BackupType::Full,
        _ => return None,
    };

    let date = parts[2].to_string();

    // Validate date format (basic check: 8 digits)
    if date.len() != 8 || !date.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }

    let type_str = backup_type.as_str().to_string();
    Some(BackupEntry {
        filename: filename.to_string(),
        backup_type,
        date,
        size_bytes: None,
        path: format!("{}/{}/{}", base_dir, type_str, filename),
    })
}

/// Calculate which backups to keep based on retention policy.
///
/// Returns filenames that should be DELETED.
pub fn calculate_retention(
    entries: &[BackupEntry],
    daily_retention: usize,
    weekly_retention: usize,
) -> Vec<String> {
    // Sort entries by date descending (newest first)
    let mut sorted: Vec<&BackupEntry> = entries.iter().collect();
    sorted.sort_by(|a, b| b.date.cmp(&a.date));

    let mut to_keep: Vec<&str> = Vec::new();
    let mut daily_kept = 0usize;
    let mut weekly_dates: Vec<String> = Vec::new();

    for entry in &sorted {
        // Keep the newest `daily_retention` backups
        if daily_kept < daily_retention {
            to_keep.push(&entry.filename);
            daily_kept += 1;
            continue;
        }

        // For older backups, keep one per week up to weekly_retention
        if let Some(week_key) = get_week_key(&entry.date) {
            if !weekly_dates.contains(&week_key) && weekly_dates.len() < weekly_retention {
                weekly_dates.push(week_key);
                to_keep.push(&entry.filename);
            }
        }
    }

    // Return filenames NOT in to_keep
    entries
        .iter()
        .filter(|e| !to_keep.contains(&e.filename.as_str()))
        .map(|e| e.filename.clone())
        .collect()
}

/// Get ISO week key from a YYYYMMDD date string.
fn get_week_key(date_str: &str) -> Option<String> {
    let date = NaiveDate::parse_from_str(date_str, "%Y%m%d").ok()?;
    let iso_week = date.iso_week();
    Some(format!("{}W{:02}", iso_week.year(), iso_week.week()))
}

/// Sort backup entries by date descending (newest first).
pub fn list_backups_sorted(entries: &mut Vec<BackupEntry>) {
    entries.sort_by(|a, b| b.date.cmp(&a.date));
}

/// Build backup status from a list of entries.
pub fn build_backup_status(config: &BackupConfig, entries: &[BackupEntry]) -> BackupStatus {
    let mut sorted = entries.to_vec();
    list_backups_sorted(&mut sorted);

    let latest = sorted.first().cloned();
    let daily_count = sorted.iter().take(config.daily_retention).count();
    let weekly_count = {
        let mut weeks: Vec<String> = Vec::new();
        for e in sorted.iter().skip(config.daily_retention) {
            if let Some(wk) = get_week_key(&e.date) {
                if !weeks.contains(&wk) {
                    weeks.push(wk);
                }
            }
        }
        weeks.len()
    };

    BackupStatus {
        enabled: true,
        backup_dir: config.backup_dir.clone(),
        total_backups: entries.len(),
        latest_backup: latest,
        daily_count,
        weekly_count,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// TDD Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> BackupConfig {
        BackupConfig {
            backup_dir: "/data/backups".to_string(),
            daily_retention: 7,
            weekly_retention: 4,
        }
    }

    // ========================================
    // UT-014b_a: generate_backup_path
    // ========================================
    #[test]
    fn test_generate_backup_path_mariadb() {
        let config = test_config();
        let path = generate_backup_path(&config, &BackupType::MariaDB);
        assert!(path.starts_with("/data/backups/mariadb/mimir_mariadb_"));
        assert!(path.ends_with(".sql.gz"));
    }

    #[test]
    fn test_generate_backup_path_qdrant() {
        let config = test_config();
        let path = generate_backup_path(&config, &BackupType::Qdrant);
        assert!(path.starts_with("/data/backups/qdrant/mimir_qdrant_"));
        assert!(path.ends_with(".snapshot"));
    }

    #[test]
    fn test_generate_backup_path_config() {
        let config = test_config();
        let path = generate_backup_path(&config, &BackupType::Config);
        assert!(path.starts_with("/data/backups/config/mimir_config_"));
        assert!(path.ends_with(".tar.gz"));
    }

    #[test]
    fn test_generate_backup_path_full() {
        let config = test_config();
        let path = generate_backup_path(&config, &BackupType::Full);
        assert!(path.starts_with("/data/backups/full/mimir_full_"));
        assert!(path.ends_with(".tar.gz"));
    }

    // ========================================
    // UT-014b_b: parse_backup_filename
    // ========================================
    #[test]
    fn test_parse_backup_filename_mariadb() {
        let entry = parse_backup_filename("mimir_mariadb_20260301_120000.sql.gz", "/data/backups")
            .expect("should parse");
        assert_eq!(entry.backup_type, BackupType::MariaDB);
        assert_eq!(entry.date, "20260301");
        assert_eq!(entry.path, "/data/backups/mariadb/mimir_mariadb_20260301_120000.sql.gz");
    }

    #[test]
    fn test_parse_backup_filename_qdrant() {
        let entry = parse_backup_filename("mimir_qdrant_20260301_120000.snapshot", "/data/backups")
            .expect("should parse");
        assert_eq!(entry.backup_type, BackupType::Qdrant);
        assert_eq!(entry.date, "20260301");
    }

    #[test]
    fn test_parse_backup_filename_invalid() {
        assert!(parse_backup_filename("random_file.txt", "/data").is_none());
        assert!(parse_backup_filename("mimir_unknown_20260301_120000.sql.gz", "/data").is_none());
        assert!(parse_backup_filename("mimir_mariadb_baddate_120000.sql.gz", "/data").is_none());
    }

    // ========================================
    // UT-014b_c: calculate_retention
    // ========================================
    #[test]
    fn test_calculate_retention_keeps_daily() {
        let entries: Vec<BackupEntry> = (1..=7)
            .map(|d| BackupEntry {
                filename: format!("mimir_mariadb_202603{:02}_120000.sql.gz", d),
                backup_type: BackupType::MariaDB,
                date: format!("202603{:02}", d),
                size_bytes: None,
                path: String::new(),
            })
            .collect();

        // All 7 fit in daily retention, nothing to delete
        let to_delete = calculate_retention(&entries, 7, 4);
        assert!(to_delete.is_empty());
    }

    #[test]
    fn test_calculate_retention_deletes_old() {
        // 10 entries, keep 3 daily, 2 weekly → 5 keep max, delete 5
        let entries: Vec<BackupEntry> = (1..=10)
            .map(|d| BackupEntry {
                filename: format!("mimir_mariadb_202602{:02}_120000.sql.gz", d),
                backup_type: BackupType::MariaDB,
                date: format!("202602{:02}", d),
                size_bytes: None,
                path: String::new(),
            })
            .collect();

        let to_delete = calculate_retention(&entries, 3, 2);
        // Should keep 3 daily (10th, 9th, 8th) + up to 2 weekly from older
        assert!(!to_delete.is_empty());
        // The newest 3 should NOT be in delete list
        assert!(!to_delete.contains(&"mimir_mariadb_20260210_120000.sql.gz".to_string()));
        assert!(!to_delete.contains(&"mimir_mariadb_20260209_120000.sql.gz".to_string()));
        assert!(!to_delete.contains(&"mimir_mariadb_20260208_120000.sql.gz".to_string()));
    }

    #[test]
    fn test_calculate_retention_empty() {
        let to_delete = calculate_retention(&[], 7, 4);
        assert!(to_delete.is_empty());
    }

    // ========================================
    // UT-014b_d: list_backups_sorted
    // ========================================
    #[test]
    fn test_list_backups_sorted() {
        let mut entries = vec![
            BackupEntry {
                filename: "a.sql.gz".to_string(),
                backup_type: BackupType::MariaDB,
                date: "20260301".to_string(),
                size_bytes: None,
                path: String::new(),
            },
            BackupEntry {
                filename: "c.sql.gz".to_string(),
                backup_type: BackupType::MariaDB,
                date: "20260303".to_string(),
                size_bytes: None,
                path: String::new(),
            },
            BackupEntry {
                filename: "b.sql.gz".to_string(),
                backup_type: BackupType::MariaDB,
                date: "20260302".to_string(),
                size_bytes: None,
                path: String::new(),
            },
        ];

        list_backups_sorted(&mut entries);
        assert_eq!(entries[0].date, "20260303");
        assert_eq!(entries[1].date, "20260302");
        assert_eq!(entries[2].date, "20260301");
    }

    // ========================================
    // build_backup_status
    // ========================================
    #[test]
    fn test_build_backup_status() {
        let config = test_config();
        let entries = vec![
            BackupEntry {
                filename: "a.sql.gz".to_string(),
                backup_type: BackupType::MariaDB,
                date: "20260303".to_string(),
                size_bytes: Some(1024),
                path: "/data/backups/mariadb/a.sql.gz".to_string(),
            },
            BackupEntry {
                filename: "b.sql.gz".to_string(),
                backup_type: BackupType::MariaDB,
                date: "20260302".to_string(),
                size_bytes: Some(2048),
                path: "/data/backups/mariadb/b.sql.gz".to_string(),
            },
        ];

        let status = build_backup_status(&config, &entries);
        assert!(status.enabled);
        assert_eq!(status.total_backups, 2);
        assert_eq!(status.latest_backup.unwrap().date, "20260303");
    }

    #[test]
    fn test_build_backup_status_empty() {
        let config = test_config();
        let status = build_backup_status(&config, &[]);
        assert_eq!(status.total_backups, 0);
        assert!(status.latest_backup.is_none());
    }

    #[test]
    fn test_backup_type_as_str() {
        assert_eq!(BackupType::MariaDB.as_str(), "mariadb");
        assert_eq!(BackupType::Qdrant.as_str(), "qdrant");
        assert_eq!(BackupType::Config.as_str(), "config");
        assert_eq!(BackupType::Full.as_str(), "full");
    }

    #[test]
    fn test_backup_config_default() {
        let config = BackupConfig::default();
        assert_eq!(config.backup_dir, "/data/backups");
        assert_eq!(config.daily_retention, 7);
        assert_eq!(config.weekly_retention, 4);
    }
}
