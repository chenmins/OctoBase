# Keck / y-websocket sync troubleshooting (Y.Map key not replicated)

## Symptom

- Multiple clients are connected via y-websocket.
- Some `Y.Map` keys never appear on other clients.
- Deleting the corresponding row from `docs` table, closing all clients, and restarting Keck makes sync normal again.

## Additional symptom: `updateTime` appears unchanged on websocket, while `updateTime1` works

If REST shows `/api/block/{workspace}/map/visit_status` updated but websocket-side code still does not observe
`updateTime = "123"`, check the following first:

1. **Workspace import actually happened**
   - `POST /api/block/{workspace}/init` returns **304** when workspace already exists, so the uploaded snapshot is not applied.
   - Re-import with overwrite semantics (delete and re-init), or use `?force=true` if your server supports it.

2. **No-op write on Yjs key**
   - Yjs does not emit a meaningful map change when writing the same value again.
   - If `updateTime` is already `"123"` in the current doc state, setting it to `"123"` again can look like “REST success” but no websocket delta.
   - Writing a new key (`updateTime1`) or a different value usually produces an event immediately.

3. **Concurrent writers overwrite `updateTime`**
   - In this snapshot, `visit_status.updateTime` is frequently updated (timestamp-like value).
   - Another connected client/service may overwrite `"123"` quickly after your write; websocket may only show the final value.

## Likely root causes in current code

### 1) Broadcast channel lag can drop updates without state repair

In `save_update`, incoming `BroadcastRawContent` messages are consumed from a Tokio broadcast receiver.
When `RecvError::Lagged(num)` happens, the code only logs and continues. Missing messages are not recovered with a state-vector sync.

### 2) Persisted incremental updates may become incomplete

`save_update` writes per-guid incremental updates to storage (`docs.update_doc`).
If updates are dropped by lag, persisted history can miss operations. After restart, newly loaded state can become a bad base and cause long-lived divergence.

### 3) High fan-in + 1-second flush interval increases pressure

Updates are collected in-memory and flushed once per second. Under bursty multi-client traffic, backlog grows quickly and increases lag risk in the broadcast receiver.

## Why deleting `docs` row + full restart helps

Deleting the broken `docs` row removes the incomplete persisted update chain.
After all clients reconnect from clean state, synchronization can converge again.

## Immediate mitigations

- Reduce burst load (fewer concurrent writers per workspace, shorter payload bursts).
- Add monitoring/alerts for `Lagged(num)` frequency.
- Reduce flush interval or add size-based flush to lower backlog.
- On lag detection, force full state resync (state-vector roundtrip) instead of continuing with dropped deltas.

## Code references

- `libs/jwst-rpc/src/context.rs` (`save_update`, `apply_change`)
- `libs/jwst-storage/src/storage/docs/database.rs` (update persistence and remote broadcast)

## Online verification checklist (after lag/rebuild fix)

1. **Watch lag logs**
   - Search for `save update thread ... lagged:` and record `lagged` count.
   - Search for `start full workspace rebuild` and `full rebuild workspace ... completed`.

2. **Check rebuild success ratio**
   - Success should have matching `start full workspace rebuild` and `completed` logs for same workspace.
   - If failures exist, inspect `full rebuild get workspace ... failed` / `sync_migration ... failed` / `flush workspace ... failed`.

3. **Confirm `docs` persistence is coherent**
   - Before/after rebuild, verify `docs` rows for the workspace are recreated and actively updated.
   - During normal operation, ensure no repeated rebuild loop for the same workspace.

4. **Functional multi-client replay test**
   - Open 3+ clients on one workspace via y-websocket.
   - Concurrently write many keys into the same `Y.Map` from different clients.
   - Close/reopen one client repeatedly while writes continue.
   - Validate every key converges on all clients after reconnect.

5. **Regression guard**
   - If lag appears but no rebuild completion is observed, treat as degraded and restart/reconnect clients for that workspace.
   - If rebuild repeatedly fails, export workspace and inspect persisted update chain offline.
