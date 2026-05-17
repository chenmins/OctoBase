#[cfg(feature = "api")]
mod blobs;
#[cfg(feature = "api")]
mod blocks;
mod doc;

use std::collections::{HashMap, HashSet};

use axum::Router;
#[cfg(feature = "api")]
use axum::{
    extract::{Json, Path},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, head, post},
};
use doc::doc_apis;
use jwst_codec::{encode_update_as_message, StateVector};
use jwst_core::{DocStorage, Workspace};
use jwst_rpc::{BroadcastChannels, BroadcastType, RpcContextImpl};
use jwst_storage::{BlobStorageType, JwstStorage, JwstStorageResult};
use tokio::sync::RwLock;

use super::{redis_sync::RedisSync, *};

#[derive(Deserialize)]
#[cfg_attr(feature = "api", derive(utoipa::IntoParams))]
pub struct Pagination {
    #[serde(default)]
    offset: usize,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    usize::MAX
}

#[derive(Serialize)]
pub struct PageData<T> {
    total: usize,
    data: T,
}

pub struct Context {
    channel: BroadcastChannels,
    storage: JwstStorage,
    webhook: Arc<std::sync::RwLock<String>>,
    redis_sync: Option<Arc<RedisSync>>,
    plain_yjs_rebuilt: RwLock<HashSet<String>>,
}

impl Context {
    pub async fn new(storage: Option<JwstStorage>) -> Self {
        let blob_storage_type = BlobStorageType::DB;

        let storage = if let Some(storage) = storage {
            info!("use external storage instance: {}", storage.database());
            Ok(storage)
        } else if dotenvy::var("USE_MEMORY_SQLITE").is_ok() {
            info!("use memory sqlite database");
            JwstStorage::new_with_migration("sqlite::memory:", blob_storage_type).await
        } else if let Ok(database_url) = dotenvy::var("DATABASE_URL") {
            info!("use external database: {}", database_url);
            JwstStorage::new_with_migration(&database_url, blob_storage_type).await
        } else {
            info!("use sqlite database: jwst.db");
            JwstStorage::new_with_sqlite("jwst", blob_storage_type).await
        }
        .expect("Cannot create database");

        // Initialize Redis sync if Redis URL is provided
        let redis_sync = if let Ok(redis_url) = dotenvy::var("REDIS_URL") {
            match RedisSync::new(&redis_url).await {
                Ok(sync) => {
                    if sync.is_connected().await {
                        info!("Redis sync enabled: {}", redis_url);
                        Some(Arc::new(sync))
                    } else {
                        warn!("Redis sync initialization failed, falling back to single-node mode");
                        None
                    }
                }
                Err(e) => {
                    warn!("Failed to initialize Redis sync: {}, falling back to single-node mode", e);
                    None
                }
            }
        } else {
            info!("Redis URL not provided, running in single-node mode");
            None
        };

        Context {
            channel: RwLock::new(HashMap::new()),
            storage,
            webhook: Arc::new(std::sync::RwLock::new(
                dotenvy::var("HOOK_ENDPOINT").unwrap_or_default(),
            )),
            plain_yjs_rebuilt: RwLock::new(HashSet::new()),
            redis_sync,
        }
    }

    fn register_webhook(&self, workspace: Workspace) -> Workspace {
        #[cfg(feature = "api")]
        if workspace.subscribe_count() == 0 {
            use blocks::BlockHistory;

            let client = reqwest::Client::new();
            let rt = tokio::runtime::Handle::current();
            let webhook = self.webhook.clone();
            let ws_id = workspace.id();
            workspace.subscribe_doc(move |_, history| {
                if history.is_empty() {
                    return;
                }
                let webhook = webhook.read().unwrap();
                if webhook.is_empty() {
                    return;
                }
                // release the lock before move webhook
                let webhook = webhook.clone();
                rt.block_on(async {
                    debug!("send {} histories to webhook {}", history.len(), webhook);
                    let resp = client
                        .post(webhook)
                        .json(
                            &history
                                .iter()
                                .map(|h| (ws_id.as_str(), h).into())
                                .collect::<Vec<BlockHistory>>(),
                        )
                        .send()
                        .await
                        .unwrap();
                    if !resp.status().is_success() {
                        error!("failed to send webhook: {}", resp.status());
                    }
                });
            });
        }
        workspace
    }

    pub fn set_webhook(&self, endpoint: String) {
        let mut write_guard = self.webhook.write().unwrap();
        *write_guard = endpoint;
    }

    pub async fn get_workspace<S>(&self, workspace_id: S) -> JwstStorageResult<Workspace>
    where
        S: AsRef<str>,
    {
        self.storage
            .get_workspace(workspace_id)
            .await
            .map(|w| self.register_webhook(w))
    }

    pub async fn init_workspace<S>(&self, workspace_id: S, data: Vec<u8>) -> JwstStorageResult
    where
        S: AsRef<str>,
    {
        self.storage.init_workspace(workspace_id, data).await
    }

    pub async fn export_workspace<S>(&self, workspace_id: S) -> JwstStorageResult<Vec<u8>>
    where
        S: AsRef<str>,
    {
        self.storage.export_workspace(workspace_id).await
    }

    pub async fn persist_workspace<S>(&self, workspace_id: S, workspace: &Workspace) -> bool
    where
        S: AsRef<str>,
    {
        let workspace_id = workspace_id.as_ref();
        match workspace.sync_migration() {
            Ok(update) => self.persist_workspace_update(workspace_id, update).await,
            Err(e) => {
                error!("persist_workspace: {} sync_migration failed: {:?}", workspace_id, e);
                false
            }
        }
    }

    pub async fn persist_workspace_update<S>(&self, workspace_id: S, update: Vec<u8>) -> bool
    where
        S: AsRef<str>,
    {
        self.storage
            .full_migrate(workspace_id.as_ref().to_string(), Some(update), true)
            .await
    }

    pub async fn persist_plain_yjs_rebuild_once<S>(&self, workspace_id: S, update: Vec<u8>, reason: &str) -> bool
    where
        S: AsRef<str>,
    {
        let workspace_id = workspace_id.as_ref().to_string();
        let mut rebuilt = self.plain_yjs_rebuilt.write().await;

        if rebuilt.contains(&workspace_id) {
            info!(
                "plain_yjs_rebuild_once: workspace={} reason={} already rebuilt; skip duplicate migrate",
                workspace_id, reason
            );
            return true;
        }

        let ok = self
            .storage
            .full_migrate(workspace_id.clone(), Some(update), true)
            .await;

        if ok {
            rebuilt.insert(workspace_id.clone());
            info!(
                "plain_yjs_rebuild_once: workspace={} reason={} persisted rebuilt update",
                workspace_id, reason
            );
        } else {
            warn!(
                "plain_yjs_rebuild_once: workspace={} reason={} failed to persist rebuilt update",
                workspace_id, reason
            );
        }

        ok
    }

    pub async fn mark_plain_yjs_rebuilt<S>(&self, workspace_id: S)
    where
        S: AsRef<str>,
    {
        self.plain_yjs_rebuilt
            .write()
            .await
            .insert(workspace_id.as_ref().to_string());
    }

    pub async fn forget_plain_yjs_rebuild<S>(&self, workspace_id: S)
    where
        S: AsRef<str>,
    {
        self.plain_yjs_rebuilt.write().await.remove(workspace_id.as_ref());
    }

    pub async fn create_workspace<S>(&self, workspace_id: S) -> JwstStorageResult<Workspace>
    where
        S: AsRef<str>,
    {
        self.storage
            .create_workspace(workspace_id)
            .await
            .map(|w| self.register_webhook(w))
    }

    pub fn get_redis_sync(&self) -> Option<Arc<RedisSync>> {
        self.redis_sync.clone()
    }

    /// Persist workspace changes to the database and broadcast them to connected
    /// WebSocket clients. This should be called after any REST API write operation
    /// to ensure data is durable and propagated to collaboration sessions.
    ///
    /// `sv_before` is the state vector captured BEFORE the modifications were made.
    pub async fn persist_and_broadcast(&self, workspace: &Workspace, sv_before: &StateVector) {
        let workspace_id = workspace.id();
        let doc_guid = workspace.doc_guid().to_string();

        // Compute the update (diff between before and after modification)
        let update = match workspace.encode_update_since(sv_before) {
            Ok(update) => update,
            Err(e) => {
                error!("failed to encode workspace update for {}: {:?}", workspace_id, e);
                return;
            }
        };

        // Skip if update is empty (no actual changes)
        // An empty v1 update is encoded as [0, 0] (zero structs, zero delete sets)
        if update.is_empty() || update == [0, 0] {
            return;
        }

        // 1. Persist to database (this also sends to DocDBStorage's remote channel,
        //    which handle_connector listens to via server_rx)
        if let Err(e) = self
            .storage
            .docs()
            .update_doc(workspace_id.clone(), doc_guid.clone(), &update)
            .await
        {
            error!("failed to persist REST API update for {}: {:?}", workspace_id, e);
        }

        // 2. Broadcast to connected WebSocket clients via the broadcast channel
        let channels = self.channel.read().await;
        if let Some(broadcast_tx) = channels.get(&workspace_id) {
            if let Ok(encoded) = encode_update_as_message(update) {
                if broadcast_tx.send(BroadcastType::BroadcastContent(encoded)).is_err() {
                    debug!("no active WebSocket receivers for workspace {}", workspace_id);
                }
            }
        }
    }
}

impl<'a> RpcContextImpl<'a> for Context {
    fn get_storage(&self) -> &JwstStorage {
        &self.storage
    }

    fn get_channel(&self) -> &BroadcastChannels {
        &self.channel
    }
}

pub fn api_handler(router: Router) -> Router {
    #[cfg(feature = "api")]
    {
        router.nest("/api", blobs::blobs_apis(blocks::blocks_apis(Router::new())))
    }
    #[cfg(not(feature = "api"))]
    {
        router
    }
}
