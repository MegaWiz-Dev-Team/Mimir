//! API handler tests (run_sql / plot) — the MCP tool contract over the engine.

use mimir_lab::api::{plot, run_sql, PlotReq, RunSqlReq};
use mimir_lab::{ingest, Engine, LabError};

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn engine_with_people() -> Engine {
    let e = Engine::in_memory().unwrap();
    ingest::ingest_csv(&e, &fixture("people.csv"), "people").unwrap();
    e
}

#[test]
fn run_sql_returns_columns_and_rows() {
    let e = engine_with_people();
    let v = run_sql(
        &e,
        &RunSqlReq {
            tenant_id: "asgard_analytics".into(),
            sql: "SELECT city, count(*) AS n FROM people GROUP BY city ORDER BY city".into(),
            row_limit: None,
        },
    )
    .unwrap();
    assert_eq!(v["columns"][0]["name"], "city");
    assert_eq!(v["columns"][1]["name"], "n");
    assert_eq!(v["row_count"], 3); // Bangkok, Chiang Mai, Khon Kaen
    assert_eq!(v["truncated"], false);
}

#[test]
fn run_sql_rejects_mutation_through_api() {
    let e = engine_with_people();
    let err = run_sql(
        &e,
        &RunSqlReq {
            tenant_id: "asgard_analytics".into(),
            sql: "DELETE FROM people".into(),
            row_limit: None,
        },
    )
    .unwrap_err();
    assert!(matches!(err, LabError::NotReadOnly(_)));
}

#[test]
fn plot_builds_bar_echarts_option() {
    let e = engine_with_people();
    let v = plot(
        &e,
        &PlotReq {
            tenant_id: "asgard_analytics".into(),
            sql: "SELECT city, count(*) AS n FROM people GROUP BY city ORDER BY city".into(),
            chart_type: "bar".into(),
            x: "city".into(),
            y: "n".into(),
        },
    )
    .unwrap();
    let opt = &v["echarts"];
    assert_eq!(opt["series"][0]["type"], "bar");
    assert_eq!(opt["xAxis"]["data"][0], "Bangkok");
    // Bangkok appears twice in the fixture
    assert_eq!(opt["series"][0]["data"][0], 2.0);
}

#[test]
fn plot_builds_pie_echarts_option() {
    let e = engine_with_people();
    let v = plot(
        &e,
        &PlotReq {
            tenant_id: "asgard_analytics".into(),
            sql: "SELECT city, count(*) AS n FROM people GROUP BY city ORDER BY city".into(),
            chart_type: "pie".into(),
            x: "city".into(),
            y: "n".into(),
        },
    )
    .unwrap();
    assert_eq!(v["echarts"]["series"][0]["type"], "pie");
    assert_eq!(v["echarts"]["series"][0]["data"][0]["name"], "Bangkok");
}

#[test]
fn plot_unknown_column_errors() {
    let e = engine_with_people();
    let err = plot(
        &e,
        &PlotReq {
            tenant_id: "asgard_analytics".into(),
            sql: "SELECT city FROM people".into(),
            chart_type: "bar".into(),
            x: "nope".into(),
            y: "city".into(),
        },
    )
    .unwrap_err();
    assert!(matches!(err, LabError::Api(_)));
}
