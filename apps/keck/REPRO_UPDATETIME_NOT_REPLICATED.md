# Reproducing "HTTP `set_map` succeeds but y-websocket peer does not see the new value"

The snapshot `apps/keck/doctor_prod_userName_wuyulong1.before_fix.bin` is a real captured
example of a workspace whose Y.Doc has become CRDT-inconsistent between the keck
in-memory store and what a y-websocket peer reconstructs from the same byte stream.

## What the user reports

- `yMap.put("updateTime", "123")` (HTTP) → `GET /api/block/.../map/visit_status/updateTime`
  returns `"123"` on the server.
- A `y-websocket` client connected to the same workspace fires `Y.Map.observe`
  for the key `updateTime` but `map.get("updateTime")` does **not** become `"123"`.
- The exact same flow with key `updateTime1` (a brand-new key) works correctly.

## Why this snapshot reproduces the bug

`visit_status.updateTime` has TWO concurrent items at the same map slot, both
with `origin_left = None`:

| client       | clock | content                |
|--------------|-------|------------------------|
| `2258317784` | `0`   | `"2026-05-13 20:59:09"` |
| `1635070698` | `20`  | `"2026-05-14 10:53:22"` |

The keck Rust codec resolves the conflict so that `parent.map["updateTime"]`
points at `(1635070698, 20)`, while a yjs client reconstructs the same chain
with `parent.map["updateTime"]` pointing at `(2258317784, 0)`. From that point
on, the two systems disagree on which item is "live".

When the server then `set_map`s `updateTime = "123"`:

1. The new item is created with `left = (1635070698, 20)` and integrated into
   the keck side; `parent.map["updateTime"]` is updated to the new item, and
   the broadcast carries an `Update` for the new item plus a `Delete` for
   `(1635070698, 20)`.
2. The yjs client receives that update, splices the new item between
   `(1635070698, 20)` and its right-neighbor on the yjs side
   (which is `(2258317784, 0)`), and marks `(1635070698, 20)` as deleted.
3. `Y.Map.observe` fires (the slot's chain changed), but yjs's
   `parent.map["updateTime"]` is still pointing at `(2258317784, 0)` (the
   right-most item of the chain on the yjs side), so `map.get("updateTime")`
   still returns the original `"2026-05-13 20:59:09"`.

For `updateTime1`, no prior item exists, so both sides agree on the new item
becoming the head of an empty chain — replication works normally.

## End-to-end reproducer

Prerequisites: built `keck` binary and Node.js >= 18.

```bash
# 1) Start keck (clean storage)
KECK_LOG=debug KECK_PORT=3030 USE_MEMORY_SQLITE=true \
  target/release/keck > /tmp/keck.log 2>&1 &

# 2) Import the corrupted snapshot
WS_ID=doctor_prod_userName_wuyulong1
curl -fsS -X POST "http://127.0.0.1:3030/api/block/${WS_ID}/init" \
  -H "Content-Type: application/octet-stream" \
  --data-binary @"apps/keck/${WS_ID}.before_fix.bin" -o /tmp/init_resp.bin

# 3) In another shell, install yjs + y-websocket and run the listener:
mkdir -p /tmp/repro && cd /tmp/repro
npm init -y && npm install yjs y-websocket ws
cat > listen.mjs <<'NODE'
import WS from 'ws'
import * as Y from 'yjs'
import { WebsocketProvider } from 'y-websocket'

const ydoc = new Y.Doc()
const provider = new WebsocketProvider(
  process.env.KECK_WS || 'ws://127.0.0.1:3030/collaboration',
  process.env.WS_ID  || 'doctor_prod_userName_wuyulong1',
  ydoc,
  { WebSocketPolyfill: WS },
)
provider.on('status', e => console.log('[ws status]', e.status))
provider.on('sync',   v => console.log('[ws sync ]', v))
const map = ydoc.getMap('visit_status')
map.observe(ev => console.log('[observe] keysChanged=', [...ev.keysChanged],
                              'snapshot=', map.toJSON()))
setInterval(() => console.log('[poll] updateTime=', JSON.stringify(map.get('updateTime')),
                              ' updateTime1=', JSON.stringify(map.get('updateTime1'))), 2000)
NODE
KECK_WS=ws://127.0.0.1:3030/collaboration WS_ID=$WS_ID node listen.mjs
```

In a third shell:

```bash
WS_ID=doctor_prod_userName_wuyulong1
# Existing key — yjs observe fires but map.get does not change
curl -fsS -X POST "http://127.0.0.1:3030/api/block/${WS_ID}/map/visit_status" \
     -H 'Content-Type: application/json' -d '{"updateTime":"123"}'
sleep 2
# New key — replication works
curl -fsS -X POST "http://127.0.0.1:3030/api/block/${WS_ID}/map/visit_status" \
     -H 'Content-Type: application/json' -d '{"updateTime1":"123"}'
```

## Reading the diagnostic logs

`KECK_LOG=debug` will surface (filter with the regex below):

```
init_workspace|set_map|ymap\._insert|change history|broadcast content|forwarding BroadcastContent
```

Key lines you should see and what they mean:

- `init_workspace[...] root map "visit_status" = {... "updateTime":"2026-05-14 10:53:22"}`
  — the **server's** view immediately after import.
- `set_map[ws.visit_status] key="updateTime" previous_value=Some("...10:53:22") new_value="123"`
  — what the HTTP handler observed before writing.
- `ymap._insert key="updateTime" left_item=Some((Id { client: 1635070698, clock: 20 }, None, false))`
  — which existing item the new value is being chained to.
- `ymap._insert key="updateTime" AFTER integrate parent.map[key]=Some((Id { client: <local>, clock: 0 }, false))`
  — confirms the server-side map slot now points at the new item.
- `change history id=(<local>,0) parent=["visit_status","updateTime"] action=Update content="123"`
  + `change history id=(1635070698,20) ... action=Delete content="2026-05-14 10:53:22"`
  — the broadcast covers BOTH the new item and the `Delete` of the old left.
- `ws->client[<id>] forwarding BroadcastContent: 156 bytes`
  — the message did go out to every connected y-websocket peer.

If, despite all of the above, the y-websocket client's `map.get("updateTime")`
does not change to `"123"` (only `observe` fires with the same value), the
workspace is in the inconsistent state described in this document.

## How to recover from a snapshot in this state

Per `SYNC_TROUBLESHOOTING.md`, the only reliable recovery today is:

1. Disconnect every client.
2. Drop the workspace's `docs` rows in the storage backend.
3. Re-init from a known-good snapshot (or let the surviving clients re-sync).

A workaround for the user's specific use case is to **always write under a
different key** (e.g. include a monotonic suffix) instead of overwriting the
poisoned `updateTime` slot.
