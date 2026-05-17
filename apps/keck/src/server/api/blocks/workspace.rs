use axum::{
    extract::{BodyStream, Path, Query},
    response::Response,
};
use futures::{
    future,
    stream::{iter, StreamExt},
};
use jwst_core::DocStorage;
use jwst_storage::JwstStorageError;
use utoipa::IntoParams;

use super::*;

/// Get a exists `Workspace` by id
/// - Return 200 Ok and `Workspace`'s data if `Workspace` is exists.
/// - Return 404 Not Found if `Workspace` not exists.
#[utoipa::path(
    get,
    tag = "Workspace",
    context_path = "/api/block",
    path = "/{workspace}",
    params(
        ("workspace", description = "workspace id"),
    ),
    responses(
        (status = 200, description = "Get workspace data", body = Workspace),
        (status = 404, description = "Workspace not found")
    )
)]
pub async fn get_workspace(Extension(context): Extension<Arc<Context>>, Path(workspace): Path<String>) -> Response {
    info!("get_workspace: {}", workspace);
    if let Ok(workspace) = context.get_workspace(&workspace).await {
        Json(workspace).into_response()
    } else {
        (StatusCode::NOT_FOUND, format!("Workspace({workspace:?}) not found")).into_response()
    }
}

/// Init a `Workspace` by id
/// - Return 200 Ok and `Workspace`'s data if init success.
/// - Return 304 Not Modified if `Workspace` is exists.
/// - Return 404 Not Found if `Workspace` is not exists (failed to import, data
///   may be corrupted).
/// - Return 500 Internal Server Error if init failed.
#[utoipa::path(
    post,
    tag = "Workspace",
    context_path = "/api/block",
    path = "/{workspace}/init",
    params(
        ("workspace", description = "workspace id"),
    ),
    request_body(
        content = BodyStream,
        content_type="application/octet-stream"
    ),
    responses(
        (status = 200, description = "Workspace init success", body = Vec<u8>),
        (status = 304, description = "Workspace is exists"),
        (status = 404, description = "Workspace not found (failed to import, data may be corrupted)"),
        (status = 500, description = "Failed to init a workspace")
    )
)]
pub async fn init_workspace(
    Extension(context): Extension<Arc<Context>>,
    Path(workspace): Path<String>,
    Query(query): Query<InitWorkspaceQuery>,
    body: BodyStream,
) -> Response {
    info!("init_workspace: {}", workspace);

    let mut has_error = false;
    let data = body
        .take_while(|x| {
            has_error = x.is_err();
            future::ready(x.is_ok())
        })
        .filter_map(|data| future::ready(data.ok()))
        .flat_map(iter)
        .collect::<Vec<u8>>()
        .await;

    info!(
        "init_workspace: {} received body size={} bytes, force={}",
        workspace,
        data.len(),
        query.force
    );

    if has_error {
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    } else if let Err(e) = context.init_workspace(&workspace, data.clone()).await {
        if matches!(e, JwstStorageError::WorkspaceExists(_)) {
            if !query.force {
                warn!(
                    "init_workspace: {} already exists; returning 304 (use ?force=true to overwrite)",
                    workspace
                );
                return StatusCode::NOT_MODIFIED.into_response();
            }

            warn!(
                "init_workspace: {} exists and force=true; deleting and re-importing",
                workspace
            );
            context.forget_plain_yjs_rebuild(&workspace).await;
            if context.storage.docs().delete_workspace(&workspace).await.is_err() {
                error!("init_workspace: {} force delete failed", workspace);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
            if let Err(e) = context.init_workspace(&workspace, data).await {
                warn!("failed to force init workspace: {}", e.to_string());
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        } else {
            warn!("failed to init workspace: {}", e.to_string());
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
        // Diagnostic: log what the workspace actually looks like right after
        // import. This is the canonical place to see whether the imported
        // binary carried nested CRDT types that may not survive a round-trip
        // through standard yjs clients.
        log_workspace_shape(&context, &workspace, "post-force-init-before-normalize").await;
        normalize_imported_workspace(&context, &workspace, "post-force-init").await;
        log_workspace_shape(&context, &workspace, "post-force-init-after-normalize").await;
        match context.export_workspace(workspace).await {
            Ok(data) => data.into_response(),
            Err(e) => {
                if matches!(e, JwstStorageError::WorkspaceNotFound(_)) {
                    return StatusCode::NOT_FOUND.into_response();
                }
                warn!("failed to init workspace: {}", e.to_string());
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    } else {
        log_workspace_shape(&context, &workspace, "post-init-before-normalize").await;
        normalize_imported_workspace(&context, &workspace, "post-init").await;
        log_workspace_shape(&context, &workspace, "post-init-after-normalize").await;
        match context.export_workspace(workspace).await {
            Ok(data) => data.into_response(),
            Err(e) => {
                if matches!(e, JwstStorageError::WorkspaceNotFound(_)) {
                    return StatusCode::NOT_FOUND.into_response();
                }
                warn!("failed to init workspace: {}", e.to_string());
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

async fn normalize_imported_workspace(context: &Context, workspace: &str, stage: &str) {
    match context.get_workspace(workspace).await {
        Ok(ws) => {
            let normalized = normalize_workspace_for_yjs(&ws, stage);
            if let Some((update, entries)) = rebuild_plain_root_maps_for_yjs_update(&ws, stage) {
                if context.persist_workspace_update(workspace, update).await {
                    context.mark_plain_yjs_rebuilt(workspace).await;
                    info!(
                        "normalize_imported_workspace[{}]: {} persisted rebuilt plain root maps ({} entries)",
                        stage, workspace, entries
                    );
                } else {
                    warn!(
                        "normalize_imported_workspace[{}]: {} failed to persist rebuilt plain root maps ({} entries)",
                        stage, workspace, entries
                    );
                }
            } else if normalized == 0 {
                info!(
                    "normalize_imported_workspace[{}]: {} no nested CRDT entries or plain-map rebuild needed",
                    stage, workspace
                );
            } else if context.persist_workspace(workspace, &ws).await {
                info!(
                    "normalize_imported_workspace[{}]: {} persisted {} normalized entries",
                    stage, workspace, normalized
                );
            } else {
                warn!(
                    "normalize_imported_workspace[{}]: {} failed to persist {} normalized entries",
                    stage, workspace, normalized
                );
            }
        }
        Err(e) => warn!(
            "normalize_imported_workspace[{}]: {} could not be opened: {}",
            stage, workspace, e
        ),
    }
}

/// Emit a structured log of the workspace's root keys and the variant of each
/// root. This is the smallest amount of information needed to diagnose REST vs
/// y-websocket inconsistencies caused by nested CRDT types in imported snapshots.
async fn log_workspace_shape(context: &Context, workspace: &str, stage: &str) {
    match context.get_workspace(workspace).await {
        Ok(ws) => {
            let keys = ws.doc_keys();
            info!("workspace_shape[{}]: {} root_keys={:?}", stage, workspace, keys);
            for k in &keys {
                if let Ok(m) = ws.get_or_create_map(k) {
                    let entries: Vec<(String, &'static str)> = m
                        .entries()
                        .map(|(ek, ev)| {
                            let variant: &'static str = match ev {
                                jwst_core::Value::Any(_) => "Any",
                                jwst_core::Value::Array(_) => "Y.Array",
                                jwst_core::Value::Map(_) => "Y.Map",
                                jwst_core::Value::Text(_) => "Y.Text",
                                _ => "Other",
                            };
                            (ek.to_string(), variant)
                        })
                        .collect();
                    let nested = entries
                        .iter()
                        .filter(|(_, v)| matches!(*v, "Y.Map" | "Y.Array" | "Y.Text"))
                        .count();
                    info!(
                        "workspace_shape[{}]: {} root={} len={} entries={:?}",
                        stage,
                        workspace,
                        k,
                        m.len(),
                        entries
                    );
                    if nested > 0 {
                        warn!(
                            "workspace_shape[{}]: {} root={} contains {} nested CRDT child(ren); these will not be observable by standard yjs/y-websocket peers if the snapshot used a non-standard binary encoding",
                            stage, workspace, k, nested
                        );
                    }
                }
            }
        }
        Err(e) => warn!(
            "workspace_shape[{}]: {} could not be re-opened: {}",
            stage, workspace, e
        ),
    }
}

#[derive(Deserialize, IntoParams, Default)]
pub struct InitWorkspaceQuery {
    /// Replace existing workspace when true.
    #[serde(default)]
    force: bool,
}

/// Export a `Workspace` by id
/// - Return 200 Ok and `Workspace`'s data if export success.
/// - Return 404 Not Found if `Workspace` is not exists.
/// - Return 500 Internal Server Error if export failed.
#[utoipa::path(
    get,
    tag = "Workspace",
    context_path = "/api/block",
    path = "/{workspace}/export",
    params(
        ("workspace", description = "workspace id"),
    ),
    responses(
        (status = 200, description = "Workspace export success", body = Vec<u8>),
        (status = 404, description = "Workspace is not exists"),
        (status = 500, description = "Failed to export a workspace")
    )
)]
pub async fn export_workspace(Extension(context): Extension<Arc<Context>>, Path(workspace): Path<String>) -> Response {
    info!("export_workspace: {}", workspace);

    match context.export_workspace(workspace).await {
        Ok(data) => data.into_response(),
        Err(e) => {
            if matches!(e, JwstStorageError::WorkspaceNotFound(_)) {
                return StatusCode::NOT_FOUND.into_response();
            }
            warn!("failed to init workspace: {}", e.to_string());
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Create a `Workspace` by id
/// - Return 200 Ok and `Workspace`'s data if init success or `Workspace` is
///   exists.
/// - Return 500 Internal Server Error if init failed.
#[utoipa::path(
    post,
    tag = "Workspace",
    context_path = "/api/block",
    path = "/{workspace}",
    params(
        ("workspace", description = "workspace id"),
    ),
    responses(
        (status = 200, description = "Return workspace data", body = Workspace),
        (status = 500, description = "Failed to init a workspace")
    )
)]
pub async fn set_workspace(Extension(context): Extension<Arc<Context>>, Path(workspace): Path<String>) -> Response {
    info!("set_workspace: {}", workspace);
    match context.create_workspace(workspace).await {
        Ok(workspace) => Json(workspace).into_response(),
        Err(e) => {
            error!("Failed to init doc: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Delete a exists `Workspace` by id
/// - Return 204 No Content if delete successful.
/// - Return 404 Not Found if `Workspace` not exists.
/// - Return 500 Internal Server Error if delete failed.
#[utoipa::path(
    delete,
    tag = "Workspace",
    context_path = "/api/block",
    path = "/{workspace}",
    params(
        ("workspace", description = "workspace id"),
    ),
    responses(
        (status = 204, description = "Workspace data deleted"),
        (status = 404, description = "Workspace not exists"),
        (status = 500, description = "Failed to delete workspace")
    )
)]
pub async fn delete_workspace(Extension(context): Extension<Arc<Context>>, Path(workspace): Path<String>) -> Response {
    info!("delete_workspace: {}", workspace);
    context.forget_plain_yjs_rebuild(&workspace).await;
    if context.storage.docs().delete_workspace(&workspace).await.is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };

    StatusCode::NO_CONTENT.into_response()
}

/// Block search query
// See doc for using utoipa search queries example here: https://github.com/juhaku/utoipa/blob/6c7f6a2d/examples/todo-axum/src/main.rs#L124-L130
#[derive(Deserialize, IntoParams)]
pub struct BlockSearchQuery {
    /// Search by title and text.
    _query: String,
}

/// Search workspace blocks of server
///
/// This will return back a list of relevant blocks.
// #[utoipa::path(
//     get,
//     tag = "Workspace",
//     context_path = "/api/search",
//     path = "/{workspace}",
//     params(
//         ("workspace", description = "workspace id"),
//         BlockSearchQuery,
//     ),
//     responses(
//         (status = 200, description = "Search results", body = SearchResults),
//     )
// )]
// pub async fn workspace_search(
//     Extension(context): Extension<Arc<Context>>,
//     Path(workspace): Path<String>,
//     query: Query<BlockSearchQuery>,
// ) -> Response { let query_text = &query.query; let ws_id = workspace; info!("workspace_search: {ws_id:?} query =
//   {query_text:?}"); if let Ok(workspace) = context.get_workspace(&ws_id).await { match workspace.search(query_text) {
//   Ok(list) => { debug!("workspace_search: {ws_id:?} query = {query_text:?}; {list:#?}"); Json(list).into_response() }
//   Err(err) => { error!("Internal server error calling workspace_search: {err:?}");
//   StatusCode::INTERNAL_SERVER_ERROR.into_response() } } } else { (StatusCode::NOT_FOUND,
//   format!("Workspace({ws_id:?}) not found")).into_response() }
// }

// #[utoipa::path(
//     get,
//     tag = "Workspace",
//     context_path = "/api/search",
//     path = "/{workspace}/index",
//     params(
//         ("workspace", description = "workspace id"),
//     ),
//     responses(
//         (status = 200, description = "result", body = Vec<String>),
//         (status = 404, description = "Workspace not found")
//     )
// )]
// pub async fn get_search_index(Extension(context): Extension<Arc<Context>>, Path(workspace): Path<String>) -> Response
// {     info!("get_search_index: {workspace:?}");

//     if let Ok(workspace) = context.get_workspace(&workspace).await {
//         Json(workspace.metadata().search_index).into_response()
//     } else {
//         (StatusCode::NOT_FOUND, format!("Workspace({workspace:?}) not found")).into_response()
//     }
// }

// #[utoipa::path(
//     post,
//     tag = "Workspace",
//     context_path = "/api/search",
//     path = "/{workspace}/index",
//     params(
//         ("workspace", description = "workspace id"),
//     ),
//     responses(
//         (status = 200, description = "success"),
//         (status = 400, description = "Bad Request"),
//         (status = 404, description = "Workspace not found")
//     )
// )]
// pub async fn set_search_index(
//     Extension(context): Extension<Arc<Context>>,
//     Path(workspace): Path<String>,
//     Json(fields): Json<Vec<String>>,
// ) -> Response { info!("set_search_index: {workspace:?} fields = {fields:?}");

//     if let Ok(workspace) = context.get_workspace(&workspace).await {
//         if let Ok(true) = workspace.set_search_index(fields) {
//             StatusCode::OK.into_response()
//         } else {
//             StatusCode::BAD_REQUEST.into_response()
//         }
//     } else {
//         (StatusCode::NOT_FOUND, format!("Workspace({workspace:?}) not found")).into_response()
//     }
// }

/// Get `Block` in `Workspace`
/// - Return 200 and `Block`'s ID.
/// - Return 404 Not Found if `Workspace` or `Block` not exists.
#[utoipa::path(
    get,
    tag = "Workspace",
    context_path = "/api/block",
    path = "/{workspace}/blocks",
    params(
        ("workspace", description = "workspace id"),
        Pagination
    ),
    responses(
        (status = 200, description = "Get Blocks", body = PageData<[Block]>),
        (status = 404, description = "Workspace or block not found"),
    )
)]
pub async fn get_workspace_block(
    Extension(context): Extension<Arc<Context>>,
    Path(workspace): Path<String>,
    Query(pagination): Query<Pagination>,
) -> Response {
    let Pagination { offset, limit } = pagination;
    info!("get_workspace_block: {workspace:?}");
    if let Ok(space) = context
        .get_workspace(&workspace)
        .await
        .and_then(|mut ws| Ok(ws.get_blocks()?))
    {
        let total = space.block_count() as usize;
        let data = space.blocks(|blocks| blocks.skip(offset).take(limit).collect::<Vec<_>>());

        let status = if data.is_empty() {
            StatusCode::NOT_FOUND
        } else {
            StatusCode::OK
        };

        (status, Json(PageData { total, data })).into_response()
    } else {
        (StatusCode::NOT_FOUND, format!("Workspace({workspace:?}) not found")).into_response()
    }
}

#[cfg(all(test, feature = "sqlite"))]
mod test {
    use super::*;

    #[tokio::test]
    async fn workspace() {
        use axum_test_helper::TestClient;

        let pool = DbPool::init_memory_pool().await.unwrap();
        let context = Arc::new(Context::new(Some(pool)).await);

        let app = super::workspace_apis(Router::new()).layer(Extension(context));

        let client = TestClient::new(app);

        let resp = client.post("/block/test").send().await;

        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(resp.json::<schema::Workspace>().await, schema::Workspace::default());

        let resp = client.get("/block/test").send().await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(resp.json::<schema::Workspace>().await, schema::Workspace::default());
    }
}
