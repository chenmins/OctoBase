use std::convert::Infallible;

use axum::{
    extract::Path,
    response::{
        sse::{Event, KeepAlive, Sse},
        Response,
    },
};
use jwst_core::Value;
use serde_json::Value as JsonValue;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use super::*;

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
            Ok(map) => Json(map).into_response(),
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
                    Json(value_to_json(&val)).into_response()
                } else {
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
                    for (k, v) in obj.iter() {
                        let any = json_to_any(v.clone());
                        if let Err(e) = map.insert(k.clone(), any) {
                            error!("failed to set map key {}: {:?}", k, e);
                            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                        }
                    }
                    Json(map).into_response()
                } else {
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
                if let Some(val) = array.get(index) {
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
                let action = payload.get("action").and_then(|v| v.as_str()).unwrap_or("push");

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
                        let index = payload.get("index").and_then(|v| v.as_u64()).unwrap_or(0);
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
        Json(ws.doc_keys()).into_response()
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
