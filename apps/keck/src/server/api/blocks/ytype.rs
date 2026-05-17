use std::convert::Infallible;

use axum::{
    extract::Path,
    response::{
        sse::{Event, KeepAlive, Sse},
        Response,
    },
};
use jwst_core::{Any, Value};
use serde_json::Value as JsonValue;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use super::*;

// ─── Diagnostic helpers ──────────────────────────────────────────────────────
// These helpers describe the variant of a `Value` (the in-memory representation
// kept by `jwst_codec`). They are intentionally cheap so they can be emitted
// from hot REST/SSE paths without measurable overhead, but provide enough info
// to diagnose REST-vs-y-websocket inconsistencies — especially nested CRDT
// types (Y.Map / Y.Array) that the standard `yjs` JS client may not be able to
// decode from a non-standard binary snapshot.

fn any_variant(any: &Any) -> &'static str {
    match any {
        Any::Undefined => "Any::Undefined",
        Any::Null => "Any::Null",
        Any::Integer(_) => "Any::Integer",
        Any::Float32(_) => "Any::Float32",
        Any::Float64(_) => "Any::Float64",
        Any::BigInt64(_) => "Any::BigInt64",
        Any::False | Any::True => "Any::Bool",
        Any::String(_) => "Any::String",
        Any::Object(_) => "Any::Object",
        Any::Array(_) => "Any::Array",
        Any::Binary(_) => "Any::Binary",
    }
}

fn value_variant(val: &Value) -> &'static str {
    match val {
        Value::Any(_) => "Any",
        Value::Array(_) => "Y.Array",
        Value::Map(_) => "Y.Map",
        Value::Text(_) => "Y.Text",
        _ => "Other",
    }
}

/// Summarise a value for logs: variant + size hint + short preview.
/// Nested CRDT types (Y.Map / Y.Array) are flagged because they are the most
/// likely source of REST-vs-yjs view inconsistency in imported snapshots.
fn value_summary(val: &Value) -> String {
    match val {
        Value::Any(any) => format!("Any({})", any_variant(any)),
        Value::Map(map) => {
            let keys: Vec<String> = map.keys().map(|k| k.to_string()).collect();
            format!("Y.Map(len={}, keys={:?})", map.len(), keys)
        }
        Value::Array(arr) => format!("Y.Array(len={})", arr.len()),
        Value::Text(text) => format!("Y.Text(len={})", text.to_string().chars().count()),
        _ => "Other".to_string(),
    }
}

// ─── Helper: convert jwst_codec::Value to serde_json::Value ──────────────────

fn value_to_json(val: &Value) -> JsonValue {
    match val {
        Value::Any(any) => serde_json::to_value(any).unwrap_or(JsonValue::Null),
        Value::Array(arr) => {
            let items: Vec<JsonValue> = arr.iter().map(|v| value_to_json(&v)).collect();
            JsonValue::Array(items)
        }
        Value::Map(map) => {
            let mut obj = serde_json::Map::new();
            for (k, v) in map.entries() {
                obj.insert(k.to_string(), value_to_json(&v));
            }
            JsonValue::Object(obj)
        }
        Value::Text(text) => JsonValue::String(text.to_string()),
        _ => JsonValue::Null,
    }
}

fn json_to_any(val: JsonValue) -> jwst_core::Any {
    jwst_core::Any::from(val)
}

// ─── YMap Endpoints ──────────────────────────────────────────────────────────

/// Get all entries of a named y.map in the workspace
/// GET /api/block/{workspace}/map/{name}
#[utoipa::path(
    get,
    tag = "YType",
    context_path = "/api/block",
    path = "/{workspace}/map/{name}",
    params(
        ("workspace", description = "workspace id"),
        ("name", description = "y.map name"),
    ),
    responses(
        (status = 200, description = "Get map entries"),
        (status = 404, description = "Workspace not found or map not found")
    )
)]
pub async fn get_map(
    Extension(context): Extension<Arc<Context>>,
    Path((workspace, name)): Path<(String, String)>,
) -> Response {
    info!("get_map: workspace={}, name={}", workspace, name);
    if let Ok(ws) = context.get_workspace(&workspace).await {
        match ws.get_or_create_map(&name) {
            Ok(map) => {
                // Diagnostic: enumerate entries so REST vs y-websocket discrepancies
                // (e.g. nested Y.Map / Y.Array that some yjs clients cannot decode
                // from a non-standard imported snapshot) are visible in the log.
                let entries: Vec<(String, String)> =
                    map.entries().map(|(k, v)| (k.to_string(), value_summary(&v))).collect();
                info!(
                    "get_map: workspace={}, name={}, len={}, entries=[{}]",
                    workspace,
                    name,
                    map.len(),
                    entries
                        .iter()
                        .map(|(k, s)| format!("{}={}", k, s))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                let nested_count = entries
                    .iter()
                    .filter(|(_, s)| s.starts_with("Y.Map") || s.starts_with("Y.Array") || s.starts_with("Y.Text"))
                    .count();
                if nested_count > 0 {
                    warn!(
                        "get_map: workspace={}, name={} contains {} nested CRDT value(s); standard yjs/y-websocket peers may not see these if the snapshot was produced by jwst-codec",
                        workspace, name, nested_count
                    );
                }
                Json(map).into_response()
            }
            Err(e) => {
                error!("failed to get map: {:?}", e);
                (StatusCode::NOT_FOUND, format!("Map({name:?}) not found")).into_response()
            }
        }
    } else {
        (StatusCode::NOT_FOUND, format!("Workspace({workspace:?}) not found")).into_response()
    }
}

/// Get a specific key from a named y.map
/// GET /api/block/{workspace}/map/{name}/{key}
#[utoipa::path(
    get,
    tag = "YType",
    context_path = "/api/block",
    path = "/{workspace}/map/{name}/{key}",
    params(
        ("workspace", description = "workspace id"),
        ("name", description = "y.map name"),
        ("key", description = "map key"),
    ),
    responses(
        (status = 200, description = "Get map value for key"),
        (status = 404, description = "Workspace, map, or key not found")
    )
)]
pub async fn get_map_key(
    Extension(context): Extension<Arc<Context>>,
    Path((workspace, name, key)): Path<(String, String, String)>,
) -> Response {
    info!("get_map_key: workspace={}, name={}, key={}", workspace, name, key);
    if let Ok(ws) = context.get_workspace(&workspace).await {
        match ws.get_or_create_map(&name) {
            Ok(map) => {
                if let Some(val) = map.get(&key) {
                    let variant = value_variant(&val);
                    info!(
                        "get_map_key: workspace={}, name={}, key={}, value_variant={}, summary={}",
                        workspace,
                        name,
                        key,
                        variant,
                        value_summary(&val)
                    );
                    if matches!(val, Value::Map(_) | Value::Array(_) | Value::Text(_)) {
                        warn!(
                            "get_map_key: workspace={}, name={}, key={} is a nested CRDT type ({}); standard yjs peers may not see this key over y-websocket — REST and websocket views will diverge",
                            workspace, name, key, variant
                        );
                    }
                    Json(value_to_json(&val)).into_response()
                } else {
                    info!(
                        "get_map_key: workspace={}, name={}, key={} not found (map len={}, existing keys={:?})",
                        workspace,
                        name,
                        key,
                        map.len(),
                        map.keys().collect::<Vec<_>>()
                    );
                    (
                        StatusCode::NOT_FOUND,
                        format!("Key({key:?}) not found in map({name:?})"),
                    )
                        .into_response()
                }
            }
            Err(e) => {
                error!("failed to get map: {:?}", e);
                (StatusCode::NOT_FOUND, format!("Map({name:?}) not found")).into_response()
            }
        }
    } else {
        (StatusCode::NOT_FOUND, format!("Workspace({workspace:?}) not found")).into_response()
    }
}

/// Set (create/update) entries in a named y.map
/// POST /api/block/{workspace}/map/{name}
/// Body: JSON object of key-value pairs to set
#[utoipa::path(
    post,
    tag = "YType",
    context_path = "/api/block",
    path = "/{workspace}/map/{name}",
    params(
        ("workspace", description = "workspace id"),
        ("name", description = "y.map name"),
    ),
    request_body(
        content = String,
        description = "JSON object with key-value pairs to set",
        content_type = "application/json"
    ),
    responses(
        (status = 200, description = "Map entries set successfully"),
        (status = 400, description = "Invalid JSON body, expected an object"),
        (status = 404, description = "Workspace not found"),
        (status = 500, description = "Failed to set map entries")
    )
)]
pub async fn set_map(
    Extension(context): Extension<Arc<Context>>,
    Path((workspace, name)): Path<(String, String)>,
    Json(payload): Json<JsonValue>,
) -> Response {
    info!("set_map: workspace={}, name={}", workspace, name);
    if let Ok(ws) = context.get_workspace(&workspace).await {
        match ws.get_or_create_map(&name) {
            Ok(mut map) => {
                if let Some(obj) = payload.as_object() {
                    let pre_len = map.len();
                    let pre_keys: Vec<String> = map.keys().map(|k| k.to_string()).collect();
                    info!(
                        "set_map: workspace={}, name={}, pre_len={}, pre_keys={:?}, incoming_keys={:?}",
                        workspace,
                        name,
                        pre_len,
                        pre_keys,
                        obj.keys().collect::<Vec<_>>()
                    );
                    for (k, v) in obj.iter() {
                        // Diagnostic: warn if we are about to silently replace a
                        // nested CRDT value (Y.Map/Y.Array/Y.Text) with a scalar
                        // Any. This is one of the main reasons the REST view and
                        // the y-websocket view stop matching after a write —
                        // jwst-codec drops the CRDT type and pushes an Any that
                        // every yjs peer will see, but any nested state that was
                        // previously hidden from yjs peers is now lost server-side
                        // as well.
                        if let Some(existing) = map.get(k) {
                            let existing_variant = value_variant(&existing);
                            if matches!(existing, Value::Map(_) | Value::Array(_) | Value::Text(_)) {
                                warn!(
                                    "set_map: workspace={}, name={}, key={} OVERWRITES nested CRDT ({}) with Any from REST JSON — y-websocket peers and REST will converge after this write, but any sub-structure of the previous {} is dropped",
                                    workspace, name, k, existing_variant, existing_variant
                                );
                            } else {
                                debug!(
                                    "set_map: workspace={}, name={}, key={} replaces existing {} value",
                                    workspace, name, k, existing_variant
                                );
                            }
                        } else {
                            debug!("set_map: workspace={}, name={}, key={} is new", workspace, name, k);
                        }

                        let any = json_to_any(v.clone());
                        let any_kind = any_variant(&any);
                        trace!(
                            "set_map: workspace={}, name={}, key={} <- {} ({})",
                            workspace,
                            name,
                            k,
                            any_kind,
                            v
                        );
                        if let Err(e) = map.insert(k.clone(), any) {
                            error!("failed to set map key {}: {:?}", k, e);
                            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                        }
                    }
                    info!(
                        "set_map: workspace={}, name={}, post_len={} (delta={})",
                        workspace,
                        name,
                        map.len(),
                        (map.len() as i64) - (pre_len as i64)
                    );
                    Json(map).into_response()
                } else {
                    warn!(
                        "set_map: workspace={}, name={} rejected — body is not a JSON object",
                        workspace, name
                    );
                    (StatusCode::BAD_REQUEST, "Expected a JSON object").into_response()
                }
            }
            Err(e) => {
                error!("failed to get/create map: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    } else {
        (StatusCode::NOT_FOUND, format!("Workspace({workspace:?}) not found")).into_response()
    }
}

/// Delete a key from a named y.map
/// DELETE /api/block/{workspace}/map/{name}/{key}
#[utoipa::path(
    delete,
    tag = "YType",
    context_path = "/api/block",
    path = "/{workspace}/map/{name}/{key}",
    params(
        ("workspace", description = "workspace id"),
        ("name", description = "y.map name"),
        ("key", description = "map key"),
    ),
    responses(
        (status = 204, description = "Key deleted from map"),
        (status = 404, description = "Workspace or map not found")
    )
)]
pub async fn delete_map_key(
    Extension(context): Extension<Arc<Context>>,
    Path((workspace, name, key)): Path<(String, String, String)>,
) -> Response {
    info!("delete_map_key: workspace={}, name={}, key={}", workspace, name, key);
    if let Ok(ws) = context.get_workspace(&workspace).await {
        match ws.get_or_create_map(&name) {
            Ok(mut map) => {
                map.remove(&key);
                StatusCode::NO_CONTENT.into_response()
            }
            Err(e) => {
                error!("failed to get map: {:?}", e);
                (StatusCode::NOT_FOUND, format!("Map({name:?}) not found")).into_response()
            }
        }
    } else {
        (StatusCode::NOT_FOUND, format!("Workspace({workspace:?}) not found")).into_response()
    }
}

// ─── YArray Endpoints ────────────────────────────────────────────────────────

/// Get all elements of a named y.array in the workspace
/// GET /api/block/{workspace}/array/{name}
#[utoipa::path(
    get,
    tag = "YType",
    context_path = "/api/block",
    path = "/{workspace}/array/{name}",
    params(
        ("workspace", description = "workspace id"),
        ("name", description = "y.array name"),
    ),
    responses(
        (status = 200, description = "Get array elements"),
        (status = 404, description = "Workspace not found or array not found")
    )
)]
pub async fn get_array(
    Extension(context): Extension<Arc<Context>>,
    Path((workspace, name)): Path<(String, String)>,
) -> Response {
    info!("get_array: workspace={}, name={}", workspace, name);
    if let Ok(ws) = context.get_workspace(&workspace).await {
        match ws.get_or_create_array(&name) {
            Ok(array) => Json(array).into_response(),
            Err(e) => {
                error!("failed to get array: {:?}", e);
                (StatusCode::NOT_FOUND, format!("Array({name:?}) not found")).into_response()
            }
        }
    } else {
        (StatusCode::NOT_FOUND, format!("Workspace({workspace:?}) not found")).into_response()
    }
}

/// Get an element at a specific index from a named y.array
/// GET /api/block/{workspace}/array/{name}/{index}
#[utoipa::path(
    get,
    tag = "YType",
    context_path = "/api/block",
    path = "/{workspace}/array/{name}/{index}",
    params(
        ("workspace", description = "workspace id"),
        ("name", description = "y.array name"),
        ("index", description = "array index"),
    ),
    responses(
        (status = 200, description = "Get array element at index"),
        (status = 404, description = "Workspace, array, or index not found")
    )
)]
pub async fn get_array_element(
    Extension(context): Extension<Arc<Context>>,
    Path((workspace, name, index)): Path<(String, String, u64)>,
) -> Response {
    info!(
        "get_array_element: workspace={}, name={}, index={}",
        workspace, name, index
    );
    if let Ok(ws) = context.get_workspace(&workspace).await {
        match ws.get_or_create_array(&name) {
            Ok(array) => {
                let val = usize::try_from(index).ok().and_then(|idx| array.iter().nth(idx));
                if let Some(val) = val {
                    Json(value_to_json(&val)).into_response()
                } else {
                    (
                        StatusCode::NOT_FOUND,
                        format!("Index({index}) out of bounds for array({name:?})"),
                    )
                        .into_response()
                }
            }
            Err(e) => {
                error!("failed to get array: {:?}", e);
                (StatusCode::NOT_FOUND, format!("Array({name:?}) not found")).into_response()
            }
        }
    } else {
        (StatusCode::NOT_FOUND, format!("Workspace({workspace:?}) not found")).into_response()
    }
}

/// Push or insert elements into a named y.array
/// POST /api/block/{workspace}/array/{name}
/// Body: { "action": "push", "value": ... }
///    or { "action": "insert", "index": 0, "value": ... }
#[utoipa::path(
    post,
    tag = "YType",
    context_path = "/api/block",
    path = "/{workspace}/array/{name}",
    params(
        ("workspace", description = "workspace id"),
        ("name", description = "y.array name"),
    ),
    request_body(
        content = String,
        description = "JSON object: { \"action\": \"push\"|\"insert\", \"value\": ..., \"index\": N (for insert) }",
        content_type = "application/json"
    ),
    responses(
        (status = 200, description = "Array modified successfully"),
        (status = 400, description = "Invalid request body"),
        (status = 404, description = "Workspace not found"),
        (status = 500, description = "Failed to modify array")
    )
)]
pub async fn modify_array(
    Extension(context): Extension<Arc<Context>>,
    Path((workspace, name)): Path<(String, String)>,
    Json(payload): Json<JsonValue>,
) -> Response {
    info!("modify_array: workspace={}, name={}", workspace, name);
    if let Ok(ws) = context.get_workspace(&workspace).await {
        match ws.get_or_create_array(&name) {
            Ok(mut array) => {
                let action = match payload.get("action").and_then(|v| v.as_str()) {
                    Some(a) => a,
                    None => {
                        return (
                            StatusCode::BAD_REQUEST,
                            "Missing \"action\" field (\"push\" or \"insert\")",
                        )
                            .into_response();
                    }
                };

                let value = match payload.get("value") {
                    Some(v) => json_to_any(v.clone()),
                    None => {
                        return (StatusCode::BAD_REQUEST, "Missing \"value\" field").into_response();
                    }
                };

                match action {
                    "push" => {
                        if let Err(e) = array.push(value) {
                            error!("failed to push to array: {:?}", e);
                            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                        }
                    }
                    "insert" => {
                        let index = match payload.get("index").and_then(|v| v.as_u64()) {
                            Some(i) => i,
                            None => {
                                return (StatusCode::BAD_REQUEST, "Missing \"index\" field for insert action")
                                    .into_response();
                            }
                        };
                        if let Err(e) = array.insert(index, value) {
                            error!("failed to insert into array: {:?}", e);
                            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                        }
                    }
                    _ => {
                        return (StatusCode::BAD_REQUEST, format!("Unknown action: {action}")).into_response();
                    }
                }

                Json(array).into_response()
            }
            Err(e) => {
                error!("failed to get/create array: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    } else {
        (StatusCode::NOT_FOUND, format!("Workspace({workspace:?}) not found")).into_response()
    }
}

/// Delete an element at a specific index from a named y.array
/// DELETE /api/block/{workspace}/array/{name}/{index}
#[utoipa::path(
    delete,
    tag = "YType",
    context_path = "/api/block",
    path = "/{workspace}/array/{name}/{index}",
    params(
        ("workspace", description = "workspace id"),
        ("name", description = "y.array name"),
        ("index", description = "array index"),
    ),
    responses(
        (status = 204, description = "Element removed from array"),
        (status = 404, description = "Workspace or array not found"),
        (status = 500, description = "Failed to remove element")
    )
)]
pub async fn delete_array_element(
    Extension(context): Extension<Arc<Context>>,
    Path((workspace, name, index)): Path<(String, String, u64)>,
) -> Response {
    info!(
        "delete_array_element: workspace={}, name={}, index={}",
        workspace, name, index
    );
    if let Ok(ws) = context.get_workspace(&workspace).await {
        match ws.get_or_create_array(&name) {
            Ok(mut array) => {
                if let Err(e) = array.remove(index, 1) {
                    error!("failed to remove from array: {:?}", e);
                    return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                }
                StatusCode::NO_CONTENT.into_response()
            }
            Err(e) => {
                error!("failed to get array: {:?}", e);
                (StatusCode::NOT_FOUND, format!("Array({name:?}) not found")).into_response()
            }
        }
    } else {
        (StatusCode::NOT_FOUND, format!("Workspace({workspace:?}) not found")).into_response()
    }
}

// ─── Doc Keys Endpoint ───────────────────────────────────────────────────────

/// List all root-level type names in the workspace doc
/// GET /api/block/{workspace}/doc/keys
#[utoipa::path(
    get,
    tag = "YType",
    context_path = "/api/block",
    path = "/{workspace}/doc/keys",
    params(
        ("workspace", description = "workspace id"),
    ),
    responses(
        (status = 200, description = "List of root-level type names", body = Vec<String>),
        (status = 404, description = "Workspace not found")
    )
)]
pub async fn get_doc_keys(Extension(context): Extension<Arc<Context>>, Path(workspace): Path<String>) -> Response {
    info!("get_doc_keys: workspace={}", workspace);
    if let Ok(ws) = context.get_workspace(&workspace).await {
        let keys = ws.doc_keys();
        // Diagnostic: for each root, attempt to inspect as a Y.Map and emit a
        // summary. If the snapshot was imported from a non-yjs-compatible binary
        // (e.g. produced by a previous jwst-codec server), this is the first
        // place where the divergence becomes visible: REST will list nested
        // CRDT children of roots that standard yjs clients can never decode.
        info!("get_doc_keys: workspace={}, root_keys={:?}", workspace, keys);
        for k in &keys {
            match ws.get_or_create_map(k) {
                Ok(m) => {
                    let entries: Vec<(String, &'static str)> = m
                        .entries()
                        .map(|(ek, ev)| (ek.to_string(), value_variant(&ev)))
                        .collect();
                    info!(
                        "get_doc_keys: workspace={}, root={} as Y.Map: len={}, entries={:?}",
                        workspace,
                        k,
                        m.len(),
                        entries
                    );
                    let nested = entries
                        .iter()
                        .filter(|(_, v)| matches!(*v, "Y.Map" | "Y.Array" | "Y.Text"))
                        .count();
                    if nested > 0 {
                        warn!(
                            "get_doc_keys: workspace={}, root={} has {} nested CRDT child(ren) — these may not be visible over y-websocket to a standard yjs client",
                            workspace, k, nested
                        );
                    }
                }
                Err(e) => {
                    debug!(
                        "get_doc_keys: workspace={}, root={} could not be opened as Y.Map: {:?}",
                        workspace, k, e
                    );
                }
            }
        }
        Json(keys).into_response()
    } else {
        (StatusCode::NOT_FOUND, format!("Workspace({workspace:?}) not found")).into_response()
    }
}

// ─── SSE Subscription Endpoint ───────────────────────────────────────────────

/// Subscribe to workspace doc changes via Server-Sent Events (SSE)
/// GET /api/block/{workspace}/subscribe/sse
///
/// Each event contains the binary update encoded as a JSON array of bytes
/// and associated history entries.
#[utoipa::path(
    get,
    tag = "YType",
    context_path = "/api/block",
    path = "/{workspace}/subscribe/sse",
    params(
        ("workspace", description = "workspace id"),
    ),
    responses(
        (status = 200, description = "SSE stream of workspace changes"),
        (status = 404, description = "Workspace not found")
    )
)]
pub async fn subscribe_sse(Extension(context): Extension<Arc<Context>>, Path(workspace): Path<String>) -> Response {
    info!("subscribe_sse: workspace={}", workspace);
    match context.get_workspace(&workspace).await {
        Ok(ws) => {
            let (tx, rx) = mpsc::channel::<Result<Event, Infallible>>(256);

            ws.subscribe_doc(move |update, histories| {
                let update_data = update.to_vec();
                let history_json: Vec<JsonValue> = histories
                    .iter()
                    .map(|h| {
                        serde_json::json!({
                            "field_name": h.field_name.as_ref().map(|s| s.to_string()),
                            "parent": h.parent.iter().map(|id| id.to_string()).collect::<Vec<String>>(),
                            "content": &h.content,
                            "action": h.action.to_string(),
                        })
                    })
                    .collect();

                let event_data = serde_json::json!({
                    "update": update_data,
                    "histories": history_json,
                });

                // Best-effort send - if receiver is dropped the subscription will stop
                let _ = tx.try_send(Ok(Event::default().event("update").data(event_data.to_string())));
            });

            let stream: ReceiverStream<Result<Event, Infallible>> = ReceiverStream::new(rx);
            Sse::new(stream).keep_alive(KeepAlive::default()).into_response()
        }
        Err(_) => (StatusCode::NOT_FOUND, format!("Workspace({workspace:?}) not found")).into_response(),
    }
}
