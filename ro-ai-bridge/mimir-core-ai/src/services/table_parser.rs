use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::fmt::Write;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Table {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl Table {
    /// Parse HTML fragment and extract all tables
    pub fn from_html(html: &str) -> Vec<Table> {
        let document = Html::parse_fragment(html);
        let table_selector = Selector::parse("table").unwrap();
        let tr_selector = Selector::parse("tr").unwrap();
        let th_selector = Selector::parse("th").unwrap();
        let td_selector = Selector::parse("td").unwrap();

        let mut tables = Vec::new();

        for table_node in document.select(&table_selector) {
            let mut headers = Vec::new();
            let mut rows = Vec::new();

            // Iterate over all rows (tr)
            for tr in table_node.select(&tr_selector) {
                // Check if this row is a header row (has 'th')
                let ths: Vec<String> = tr
                    .select(&th_selector)
                    .map(|th| th.text().collect::<Vec<_>>().join(" ").trim().to_string())
                    .collect();

                if !ths.is_empty() {
                    // It's a header row
                    if headers.is_empty() {
                        headers = ths;
                    } else {
                        // If we already have headers, this might be a complex table or second header?
                        // For MVP, simplistic handling: treat as data row if headers exist?
                        // Actually, some tables put th in tbody. Let's stick to first th set as headers.
                        // Or just append to rows if headers are already set?
                        // Let's treat subsequent th-only rows as regular rows for now to avoid data loss.
                        rows.push(ths);
                    }
                    continue;
                }

                // Regular cells (td)
                let tds: Vec<String> = tr
                    .select(&td_selector)
                    .map(|td| td.text().collect::<Vec<_>>().join(" ").trim().to_string())
                    .collect();

                if !tds.is_empty() {
                    rows.push(tds);
                }
            }

            // Normalize row lengths (Markdown tables need matching columns)
            let max_cols = headers
                .len()
                .max(rows.iter().map(|r| r.len()).max().unwrap_or(0));

            // Pad headers if missing
            while headers.len() < max_cols {
                headers.push("".to_string());
            }

            // Pad rows
            for row in &mut rows {
                while row.len() < max_cols {
                    row.push("".to_string());
                }
            }

            if !headers.is_empty() || !rows.is_empty() {
                tables.push(Table { headers, rows });
            }
        }

        tables
    }

    /// Convert Table struct to Markdown format
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();

        // 1. Headers
        if self.headers.is_empty() && self.rows.is_empty() {
            return String::new();
        }

        // If headers are empty but rows exist, generate dummy headers or empty space?
        // Markdown tables require a header line.
        let display_headers = if self.headers.is_empty() {
            // Generate empty headers based on first row length
            match self.rows.first() {
                Some(row) => vec!["".to_string(); row.len()],
                None => return String::new(),
            }
        } else {
            self.headers.clone()
        };

        if !display_headers.is_empty() {
            write!(&mut md, "| {} |\n", display_headers.join(" | ")).unwrap();

            // 2. Separator
            let separator: Vec<String> =
                display_headers.iter().map(|_| "---".to_string()).collect();
            write!(&mut md, "| {} |\n", separator.join(" | ")).unwrap();
        }

        // 3. Rows
        for row in &self.rows {
            // Clean newlines in cells to prevent breaking markdown table
            let clean_row: Vec<String> = row
                .iter()
                .map(|cell| cell.replace("\n", "<br>").replace("|", "\\|"))
                .collect();
            write!(&mut md, "| {} |\n", clean_row.join(" | ")).unwrap();
        }

        md
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_table() {
        let html = r#"
            <table>
                <tr>
                    <th>Name</th>
                    <th>Age</th>
                </tr>
                <tr>
                    <td>Alice</td>
                    <td>24</td>
                </tr>
                <tr>
                    <td>Bob</td>
                    <td>30</td>
                </tr>
            </table>
        "#;

        let tables = Table::from_html(html);
        assert_eq!(tables.len(), 1);
        let table = &tables[0];

        assert_eq!(table.headers, vec!["Name", "Age"]);
        assert_eq!(table.rows.len(), 2);
        assert_eq!(table.rows[0], vec!["Alice", "24"]);
        assert_eq!(table.rows[1], vec!["Bob", "30"]);

        let markdown = table.to_markdown();
        println!("{}", markdown);
        assert!(markdown.contains("| Name | Age |"));
        assert!(markdown.contains("| --- | --- |"));
        assert!(markdown.contains("| Alice | 24 |"));
    }

    #[test]
    fn test_table_without_header() {
        let html = r#"
            <table>
                <tr>
                    <td>Item A</td>
                    <td>100 Zeny</td>
                </tr>
                <tr>
                    <td>Item B</td>
                    <td>200 Zeny</td>
                </tr>
            </table>
        "#;

        let tables = Table::from_html(html);
        assert_eq!(tables.len(), 1);
        let table = &tables[0];
        assert_eq!(table.headers, vec!["", ""]);
        assert_eq!(table.rows.len(), 2);

        let markdown = table.to_markdown();
        println!("{}", markdown);
        // Should generate empty headers
        assert!(markdown.contains("|  |  |"));
        assert!(markdown.contains("| --- | --- |"));
        assert!(markdown.contains("| Item A | 100 Zeny |"));
    }
}
