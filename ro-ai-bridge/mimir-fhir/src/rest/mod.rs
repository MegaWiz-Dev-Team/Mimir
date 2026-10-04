//! FHIR R5 REST subset over the versioned [`Store`] — Nótt Local L1-1.
//!
//! Paths are relative to where the host nests the router (normally `/fhir`):
//!
//! | interaction | request |
//! |---|---|
//! | `search` | `GET /{type}?param=value` — exactly the parameters [`Store::search`] supports |
//! | `create` | `POST /{type}` — the server assigns the id; a client id is ignored (FHIR) |
//! | `read` | `GET /{type}/{id}` |
//! | `update` | `PUT /{type}/{id}` — `If-Match: W/"<versionId>"` required (configurable) |
//! | `history` | `GET /{type}/{id}/_history` — newest first |
//! | `vread` | `GET /{type}/{id}/_history/{vid}` |
//!
//! A response carrying one resource has `ETag: W/"<versionId>"` and `Last-Modified`.
//! Every error is an `OperationOutcome`: English `diagnostics`, Thai `details.text`.
//!
//! **Fail closed.** The router has no authentication of its own: every request must
//! carry an [`Agent`] extension, inserted by the host's auth layer, or it is answered
//! 401 before anything is read or written. The agent is recorded on every write.
//!
//! **Rules no client can bypass here:**
//! - `Provenance` and `AuditEvent` are read-only (the sign flow and the audit layer write them);
//! - a `DiagnosticReport` cannot be written in a signed status (`final`, `amended`,
//!   `corrected`, `appended`), and a signed report cannot be overwritten — both are the
//!   sign flow's job.
//!
//! Unsupported search parameters, modifiers and comma (OR) values are 400, never
//! ignored; bodies are parsed strictly (unknown fields are 400).

use std::future::Future;
use std::sync::{Arc, Mutex, PoisonError};

use axum::body::Bytes;
use axum::extract::rejection::{BytesRejection, QueryRejection};
use axum::extract::{FromRequestParts, Path, Query, State};
use axum::http::request::Parts;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use serde_json::{json, Value};

use crate::resources::{
    AuditEvent, Binary, Consent, Device, DiagnosticReport, DocumentReference, Encounter,
    FhirResource, Observation, Organization, Patient, Practitioner, PractitionerRole, Provenance,
    ServiceRequest, Task,
};
use crate::store::{Store, StoreError};

/// Media type of every response body.
pub const FHIR_JSON: &str = "application/fhir+json";

/// Types written only by server flows, never through this router.
const READ_ONLY: [&str; 2] = ["Provenance", "AuditEvent"];

/// `DiagnosticReport.status` values that only the sign flow may write.
const SIGNED: [&str; 4] = ["final", "amended", "corrected", "appended"];

/// Who is making the request (`Practitioner/…`, `PractitionerRole/…`, `Device/…`).
/// The host's auth layer inserts it as a request extension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Agent(pub String);

/// Router options.
#[derive(Debug, Clone)]
pub struct RestConfig {
    /// Base for `Location` and `Bundle.entry.fullUrl`: where the host nests the router
    /// (`/fhir`), or an absolute URL (`https://nott.local/fhir`).
    pub base: String,
    /// Refuse `PUT` without `If-Match` with 428 (lost-update protection). Default `true`.
    pub require_if_match: bool,
}

impl Default for RestConfig {
    fn default() -> Self {
        Self {
            base: "/fhir".into(),
            require_if_match: true,
        }
    }
}

/// The store shared between requests.
pub type SharedStore = Arc<Mutex<Store>>;

#[derive(Debug, Clone)]
struct AppState {
    store: SharedStore,
    config: Arc<RestConfig>,
}

/// The `/fhir/*` router. Nest it at [`RestConfig::base`], behind the auth layer that
/// inserts [`Agent`].
pub fn router(store: SharedStore, config: RestConfig) -> Router {
    Router::new()
        .route("/{rtype}", get(search).post(create))
        .route("/{rtype}/{id}", get(read).put(update))
        .route("/{rtype}/{id}/_history", get(history))
        .route("/{rtype}/{id}/_history/{vid}", get(vread))
        .fallback(|| async { RestError::not_found("path") })
        .with_state(AppState {
            store,
            config: Arc::new(config),
        })
}

/// An error, answered as an `OperationOutcome`.
#[derive(Debug)]
pub struct RestError {
    status: StatusCode,
    /// FHIR `issue-type` code.
    code: &'static str,
    /// Text for the person in front of the screen.
    thai: &'static str,
    /// Technical detail for the caller's developer.
    diagnostics: String,
}

impl RestError {
    fn new(status: StatusCode, code: &'static str, thai: &'static str, diag: String) -> Self {
        Self {
            status,
            code,
            thai,
            diagnostics: diag,
        }
    }

    fn not_found(what: &str) -> Self {
        let thai = "ไม่พบข้อมูลที่ร้องขอ";
        Self::new(
            StatusCode::NOT_FOUND,
            "not-found",
            thai,
            format!("{what} not found"),
        )
    }

    fn not_supported(status: StatusCode, diag: String) -> Self {
        Self::new(status, "not-supported", "ระบบไม่รองรับคำขอนี้", diag)
    }

    fn invalid(diag: String) -> Self {
        let thai = "คำขอไม่ถูกต้อง";
        Self::new(StatusCode::BAD_REQUEST, "invalid", thai, diag)
    }

    fn structure(diag: String) -> Self {
        let thai = "ข้อมูลที่ส่งมามีรูปแบบไม่ถูกต้อง";
        Self::new(StatusCode::BAD_REQUEST, "structure", thai, diag)
    }

    fn business_rule(diag: String) -> Self {
        let thai = "ทำรายการนี้ไม่ได้ตามกฎของระบบ";
        Self::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "business-rule",
            thai,
            diag,
        )
    }

    fn internal(diag: String) -> Self {
        let thai = "ระบบขัดข้อง กรุณาลองใหม่หรือแจ้งผู้ดูแลระบบ";
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "exception", thai, diag)
    }
}

impl From<StoreError> for RestError {
    fn from(e: StoreError) -> Self {
        let diag = e.to_string();
        match e {
            StoreError::NotFound(rtype, id) => Self::not_found(&format!("{rtype}/{id}")),
            StoreError::VersionConflict { .. } => Self::new(
                StatusCode::PRECONDITION_FAILED,
                "conflict",
                "ข้อมูลถูกแก้ไขไปแล้ว กรุณาโหลดฉบับล่าสุดก่อนแก้ไข",
                diag,
            ),
            StoreError::AlreadyExists(..) => {
                Self::new(StatusCode::CONFLICT, "duplicate", "มีข้อมูลนี้อยู่แล้ว", diag)
            }
            StoreError::UnsupportedSearchParam(_) => {
                Self::not_supported(StatusCode::BAD_REQUEST, diag)
            }
            StoreError::MissingAgent => unauthenticated(),
            StoreError::Sqlite(_) | StoreError::Json(_) | StoreError::Invariant(_) => {
                Self::internal(diag)
            }
        }
    }
}

impl From<serde_json::Error> for RestError {
    fn from(e: serde_json::Error) -> Self {
        Self::internal(e.to_string())
    }
}

impl IntoResponse for RestError {
    fn into_response(self) -> Response {
        let outcome = json!({
            "resourceType": "OperationOutcome",
            "issue": [{
                "severity": "error",
                "code": self.code,
                "details": { "text": self.thai },
                "diagnostics": self.diagnostics,
            }],
        });
        fhir_json(self.status, &outcome)
    }
}

fn unauthenticated() -> RestError {
    RestError::new(
        StatusCode::UNAUTHORIZED,
        "login",
        "กรุณาเข้าสู่ระบบ",
        "no authenticated agent on the request".into(),
    )
}

impl<S: Send + Sync> FromRequestParts<S> for Agent {
    type Rejection = RestError;

    fn from_request_parts(
        parts: &mut Parts,
        _: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        std::future::ready(
            parts
                .extensions
                .get::<Agent>()
                .filter(|a| !a.0.trim().is_empty())
                .cloned()
                .ok_or_else(unauthenticated),
        )
    }
}

type Reply = Result<Response, RestError>;

/// Runs `$body` with `$T` bound to the resource type named by `$rtype`.
macro_rules! for_type {
    ($rtype:expr, $T:ident => $body:expr) => {
        for_type!(@arms $rtype, $T, $body, Patient, Encounter, Organization, Practitioner,
            PractitionerRole, ServiceRequest, Task, Observation, DiagnosticReport, Binary,
            DocumentReference, Device, Provenance, AuditEvent, Consent)
    };
    (@arms $rtype:expr, $T:ident, $body:expr, $($ty:ident),+) => {
        match $rtype {
            $(stringify!($ty) => {
                type $T = $ty;
                $body
            })+
            other => Err(RestError::not_supported(
                StatusCode::NOT_FOUND,
                format!("resource type {other:?} is not supported"),
            )),
        }
    };
}

/// Runs a store operation off the async runtime, under the store lock.
///
/// A poisoned lock is recovered: a panic inside a write unwinds through the open
/// transaction, which rolls back, so the connection is consistent.
async fn run<F>(st: AppState, f: F) -> Response
where
    F: FnOnce(&mut Store, &RestConfig) -> Reply + Send + 'static,
{
    let joined = tokio::task::spawn_blocking(move || {
        let mut store = st.store.lock().unwrap_or_else(PoisonError::into_inner);
        f(&mut store, &st.config)
    })
    .await;
    match joined {
        Ok(reply) => reply.unwrap_or_else(IntoResponse::into_response),
        Err(e) => RestError::internal(e.to_string()).into_response(),
    }
}

async fn read(
    State(st): State<AppState>,
    Agent(_): Agent,
    Path((rtype, id)): Path<(String, String)>,
) -> Response {
    run(
        st,
        move |s, _| for_type!(rtype.as_str(), T => do_read::<T>(s, &id)),
    )
    .await
}

async fn vread(
    State(st): State<AppState>,
    Agent(_): Agent,
    Path((rtype, id, vid)): Path<(String, String, String)>,
) -> Response {
    run(st, move |s, _| {
        let v: u64 = vid
            .parse()
            .map_err(|_| RestError::invalid(format!("version {vid:?} is not a number")))?;
        for_type!(rtype.as_str(), T => do_vread::<T>(s, &id, v))
    })
    .await
}

async fn history(
    State(st): State<AppState>,
    Agent(_): Agent,
    Path((rtype, id)): Path<(String, String)>,
) -> Response {
    run(
        st,
        move |s, c| for_type!(rtype.as_str(), T => do_history::<T>(s, &id, &c.base)),
    )
    .await
}

async fn search(
    State(st): State<AppState>,
    Agent(_): Agent,
    Path(rtype): Path<String>,
    query: Result<Query<Vec<(String, String)>>, QueryRejection>,
) -> Response {
    run(st, move |s, c| {
        let Query(params) = query.map_err(|e| RestError::invalid(e.body_text()))?;
        if let Some((name, _)) = params.iter().find(|(_, v)| v.contains(',')) {
            return Err(RestError::not_supported(
                StatusCode::BAD_REQUEST,
                format!("comma (OR) values are not supported for {name:?}"),
            ));
        }
        for_type!(rtype.as_str(), T => do_search::<T>(s, &params, &c.base))
    })
    .await
}

async fn create(
    State(st): State<AppState>,
    Agent(agent): Agent,
    Path(rtype): Path<String>,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    run(st, move |s, c| {
        let body = parse_body(&rtype, &headers, body)?;
        for_type!(rtype.as_str(), T => do_create::<T>(s, body, &agent, &c.base))
    })
    .await
}

async fn update(
    State(st): State<AppState>,
    Agent(agent): Agent,
    Path((rtype, id)): Path<(String, String)>,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    run(st, move |s, c| {
        let body = parse_body(&rtype, &headers, body)?;
        let if_match = match headers.get(header::IF_MATCH) {
            Some(v) => Some(parse_if_match(v)?),
            None if c.require_if_match => {
                return Err(RestError::new(
                    StatusCode::PRECONDITION_REQUIRED,
                    "required",
                    "ต้องระบุฉบับที่กำลังแก้ไข กรุณาโหลดข้อมูลล่าสุดก่อนแก้ไข",
                    "PUT requires If-Match: W/\"<versionId>\"".into(),
                ))
            }
            None => None,
        };
        for_type!(rtype.as_str(), T => do_update::<T>(s, &id, body, if_match, &agent))
    })
    .await
}

fn do_read<T: FhirResource>(s: &Store, id: &str) -> Reply {
    let r = s
        .read::<T>(id)?
        .ok_or_else(|| RestError::not_found(&format!("{}/{id}", T::RESOURCE_TYPE)))?;
    one(StatusCode::OK, &r, None)
}

fn do_vread<T: FhirResource>(s: &Store, id: &str, v: u64) -> Reply {
    let r = s
        .vread::<T>(id, v)?
        .ok_or_else(|| RestError::not_found(&format!("{}/{id}/_history/{v}", T::RESOURCE_TYPE)))?;
    one(StatusCode::OK, &r, None)
}

fn do_history<T: FhirResource>(s: &Store, id: &str, base: &str) -> Reply {
    let versions = s.history::<T>(id)?;
    if versions.is_empty() {
        return Err(RestError::not_found(&format!("{}/{id}", T::RESOURCE_TYPE)));
    }
    let rtype = T::RESOURCE_TYPE;
    let entries = versions
        .iter()
        .map(|r| {
            let (v, ts) = version_meta(r);
            // `create` writes version 1 and only `update` writes the rest (store invariant).
            let (method, url, status) = if v == "1" {
                ("POST", rtype.to_string(), "201")
            } else {
                ("PUT", format!("{rtype}/{id}"), "200")
            };
            Ok(json!({
                "fullUrl": format!("{base}/{rtype}/{id}"),
                "resource": serde_json::to_value(r)?,
                "request": { "method": method, "url": url },
                "response": { "status": status, "etag": etag(&v), "lastModified": ts },
            }))
        })
        .collect::<Result<Vec<_>, RestError>>()?;
    Ok(fhir_json(StatusCode::OK, &bundle("history", entries)))
}

fn do_search<T: FhirResource>(s: &Store, params: &[(String, String)], base: &str) -> Reply {
    let pairs: Vec<(&str, &str)> = params
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let entries = s
        .search::<T>(&pairs)?
        .iter()
        .map(|r| {
            let id = r.id().map(ToString::to_string).unwrap_or_default();
            Ok(json!({
                "fullUrl": format!("{base}/{}/{id}", T::RESOURCE_TYPE),
                "resource": serde_json::to_value(r)?,
                "search": { "mode": "match" },
            }))
        })
        .collect::<Result<Vec<_>, RestError>>()?;
    Ok(fhir_json(StatusCode::OK, &bundle("searchset", entries)))
}

fn do_create<T: FhirResource>(s: &mut Store, mut body: Value, agent: &str, base: &str) -> Reply {
    if let Some(o) = body.as_object_mut() {
        o.remove("id");
    }
    guard_report(T::RESOURCE_TYPE, &body, None)?;
    let stored = s.create(typed::<T>(body)?, agent)?;
    let id = stored.id().map(ToString::to_string).unwrap_or_default();
    let (v, _) = version_meta(&stored);
    let location = format!("{base}/{}/{id}/_history/{v}", T::RESOURCE_TYPE);
    one(StatusCode::CREATED, &stored, Some(&location))
}

fn do_update<T: FhirResource>(
    s: &mut Store,
    id: &str,
    body: Value,
    if_match: Option<u64>,
    agent: &str,
) -> Reply {
    if body.get("id").and_then(Value::as_str) != Some(id) {
        return Err(RestError::invalid(format!(
            "body id must equal the URL id {id:?}"
        )));
    }
    let current = s.read::<T>(id)?.ok_or_else(|| {
        RestError::not_supported(
            StatusCode::METHOD_NOT_ALLOWED,
            format!(
                "{}/{id} does not exist; update-as-create is not supported, use POST",
                T::RESOURCE_TYPE
            ),
        )
    })?;
    guard_report(
        T::RESOURCE_TYPE,
        &body,
        Some(&serde_json::to_value(&current)?),
    )?;
    // Without a client If-Match, pin the write to the version the guard just checked.
    let expected = match if_match {
        Some(v) => v,
        None => version_meta(&current)
            .0
            .parse()
            .map_err(|_| RestError::internal("stored versionId is not a number".into()))?,
    };
    let stored = s.update(typed::<T>(body)?, Some(expected), agent)?;
    one(StatusCode::OK, &stored, None)
}

/// Content type, JSON syntax and `resourceType` of a write body; read-only types refused.
fn parse_body(
    rtype: &str,
    headers: &HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Result<Value, RestError> {
    if READ_ONLY.contains(&rtype) {
        return Err(RestError::not_supported(
            StatusCode::METHOD_NOT_ALLOWED,
            format!("{rtype} is written by the server only"),
        ));
    }
    let media = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|v| {
            v.split(';')
                .next()
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase()
        });
    if !matches!(media.as_deref(), Some(FHIR_JSON | "application/json")) {
        return Err(RestError::not_supported(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            format!("Content-Type must be {FHIR_JSON}, got {media:?}"),
        ));
    }
    let bytes = body.map_err(|e| RestError::invalid(e.body_text()))?;
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|e| RestError::structure(e.to_string()))?;
    let declared = value.get("resourceType").and_then(Value::as_str);
    if declared != Some(rtype) {
        return Err(RestError::structure(format!(
            "resourceType {declared:?} does not match the URL type {rtype:?}"
        )));
    }
    Ok(value)
}

fn typed<T: FhirResource>(body: Value) -> Result<T, RestError> {
    serde_json::from_value(body).map_err(|e| RestError::structure(e.to_string()))
}

/// Signed `DiagnosticReport` statuses are reachable only through the sign flow.
fn guard_report(rtype: &str, new: &Value, current: Option<&Value>) -> Result<(), RestError> {
    let status = |v: &Value| v.get("status").and_then(Value::as_str).map(str::to_owned);
    if rtype != "DiagnosticReport" {
        return Ok(());
    }
    if let Some(s) = status(new).filter(|s| SIGNED.contains(&s.as_str())) {
        return Err(RestError::business_rule(format!(
            "DiagnosticReport status {s:?} is set only by the sign flow"
        )));
    }
    if let Some(s) = current
        .and_then(status)
        .filter(|s| SIGNED.contains(&s.as_str()))
    {
        return Err(RestError::business_rule(format!(
            "DiagnosticReport is {s}; changes go through the sign flow (amend)"
        )));
    }
    Ok(())
}

fn parse_if_match(v: &HeaderValue) -> Result<u64, RestError> {
    let raw = v.to_str().unwrap_or_default().trim();
    raw.strip_prefix("W/")
        .unwrap_or(raw)
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .and_then(|n| n.parse().ok())
        .ok_or_else(|| {
            RestError::invalid(format!("If-Match must be W/\"<versionId>\", got {raw:?}"))
        })
}

/// `(versionId, lastUpdated)` as stored by the store.
fn version_meta<T: FhirResource>(r: &T) -> (String, String) {
    let meta = r.meta();
    let v = meta.and_then(|m| m.version_id.clone()).unwrap_or_default();
    let ts = meta
        .and_then(|m| m.last_updated.as_ref())
        .map(|t| t.as_ref().to_string())
        .unwrap_or_default();
    (v, ts)
}

fn etag(version: &str) -> String {
    format!("W/\"{version}\"")
}

fn one<T: FhirResource>(status: StatusCode, r: &T, location: Option<&str>) -> Reply {
    let mut res = fhir_json(status, &serde_json::to_value(r)?);
    let (v, ts) = version_meta(r);
    let headers = res.headers_mut();
    let mut set = |name: header::HeaderName, value: &str| -> Result<(), RestError> {
        let h = HeaderValue::from_str(value)
            .map_err(|e| RestError::internal(format!("header {name}: {e}")))?;
        headers.insert(name, h);
        Ok(())
    };
    set(header::ETAG, &etag(&v))?;
    if let Ok(t) = chrono::DateTime::parse_from_rfc3339(&ts) {
        let utc = t.with_timezone(&chrono::Utc);
        set(
            header::LAST_MODIFIED,
            &utc.format("%a, %d %b %Y %H:%M:%S GMT").to_string(),
        )?;
    }
    if let Some(l) = location {
        set(header::LOCATION, l)?;
    }
    Ok(res)
}

fn bundle(kind: &str, entries: Vec<Value>) -> Value {
    let mut b = json!({ "resourceType": "Bundle", "type": kind, "total": entries.len() });
    // FHIR JSON forbids empty arrays: no matches = no `entry`.
    if !entries.is_empty() {
        b["entry"] = Value::Array(entries);
    }
    b
}

fn fhir_json(status: StatusCode, body: &Value) -> Response {
    (
        status,
        [(header::CONTENT_TYPE, FHIR_JSON)],
        body.to_string(),
    )
        .into_response()
}
