# Keck / y-websocket sync troubleshooting (Y.Map key not replicated)

## Symptom

- Multiple clients are connected via y-websocket.
- Some `Y.Map` keys never appear on other clients.
- Deleting the corresponding row from `docs` table, closing all clients, and restarting Keck makes sync normal again.

## Additional symptom: REST `/map/visit_status` value disagrees with y-websocket subscriber

If `GET /api/block/{workspace}/map/visit_status` returns a different shape than what a
y-websocket / yjs client observes for "`visit_status`", first check that **both sides are
reading the same y-type at the same path** before suspecting a sync bug.

The new `*[diag]` log lines emitted by Keck make this trivial to verify. After importing
`doctor_prod_userName_wuyulong1.before_fix.bin` you will see, for example:

```
init_workspace[diag]: post-import workspace=doctor_prod_userName_wuyulong1
    doc_guid=doctor_prod_userName_wuyulong1 client_id=8237188974
    root_keys(count=5)=["meeting", "config", "space:updated", "visit_status", "space:meta"]
    visit_status=(root y-map, len=5, keys=["updateTime1", "aaa2", "visit_status", "aaa", "updateTime"])

get_map[diag]: workspace=doctor_prod_userName_wuyulong1 ... requested_map=visit_status
    map_present_at_root=true
get_map[diag]: workspace=doctor_prod_userName_wuyulong1 map=visit_status (root)
    len=5 keys=["updateTime1", "aaa2", "visit_status", "aaa", "updateTime"]
```

Note that the root y-map named `visit_status` itself contains a **nested key also named
`visit_status`** whose value is another `Y.Map` (with `closedVisitId`, `isVisit`,
`closedVisitStatus`, `newVisitStatus`, `newVisitId`). This is the classic source of the
"values don't agree" report:

| Reader                                                                 | What it sees                                              |
| ---------------------------------------------------------------------- | --------------------------------------------------------- |
| `GET /api/block/{ws}/map/visit_status` (REST, root)                    | The outer 5-key map (`updateTime`, `aaa`, ..., `visit_status` = nested map) |
| `ydoc.getMap('visit_status')` (yjs/y-websocket, root)                  | Same outer 5-key map                                      |
| `ydoc.getMap('visit_status').get('visit_status')` (yjs, nested Y.Map)  | The inner 5-key map with `closedVisitId` etc.             |
| A client that does `observeDeep` on a sub-map handle obtained earlier  | Only events for the *handle* it kept; sibling root writes look "missing" |

Things to verify with the new logs before treating this as a sync bug:

1. **Same workspace, same doc**
   - On REST: `get_map[diag]: ... doc_guid=<G> client_id=<C> root_keys=[...]`
   - On WS upgrade: `ws upgrade_handler[diag]: ... doc_guid=<G> client_id=<C> root_keys=[...]`
   - `doc_guid` MUST match. If not, the two sides are reading different docs.

2. **Same y-type / same path**
   - The REST handler always reads the **root** y-map called `<name>`.
   - If the y-websocket client is listening on a nested map (e.g. it stored a `Y.Map`
     handle returned by `ydoc.getMap('visit_status').get('visit_status')`), then writes
     into the *outer* `visit_status` map (or vice versa) are by design invisible to that
     listener.
   - The new `update[diag]: ... touched_root_types=[...] touches_visit_status_root=...`
     log line in `libs/jwst-rpc/src/broadcast.rs` reports which **root** y-types each
     broadcast update modifies. A line such as
     `update[diag]: ... touched_root_types=["space:something"] touches_visit_status_root=false`
     while a REST write to `visit_status` succeeds proves the two sides are operating
     on different paths.

3. **Workspace import actually happened**
   - `POST /api/block/{workspace}/init` returns **304** when workspace already exists, so
     the uploaded snapshot is not applied. The new
     `init_workspace[diag]: ... already exists and force=false ... returning 304` warning
     fires in that case. Re-import with `?force=true`.

4. **No-op write on Yjs key**
   - Yjs does not emit a meaningful map change when writing the same value again.
   - If `updateTime` is already `"123"` in the current doc state, setting it to `"123"`
     again can look like "REST success" but no websocket delta.
   - Writing a new key (`updateTime1`) or a different value usually produces an event
     immediately.

5. **Concurrent writers overwrite `updateTime`**
   - In this snapshot, `visit_status.updateTime` is frequently updated (timestamp-like value).
   - Another connected client/service may overwrite `"123"` quickly after your write;
     websocket may only show the final value.

## Likely root causes in current code (when the divergence is *not* due to path mismatch)

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

- `apps/keck/src/server/api/blocks/ytype.rs` (REST `get_map` / `get_map_key` — `*[diag]` logs of doc_guid, root keys, map keys)
- `apps/keck/src/server/api/blocks/workspace.rs` (`init_workspace` — `*[diag]` logs of received bytes, post-import shape, `force=false` 304 warning)
- `apps/keck/src/server/sync/collaboration.rs` (`upgrade_handler` — `*[diag]` log of doc_guid / root keys at WS connect time)
- `libs/jwst-rpc/src/broadcast.rs` (`subscribe_doc` callback — `update[diag]` log of touched root y-types per update)
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
