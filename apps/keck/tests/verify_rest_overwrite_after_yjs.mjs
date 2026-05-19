// Regression test for the "REST POST -> yjs client loses MeetValueAttrObj" bug.
//
// Reproduces the scenario reported by the user:
//   1. Load the binary snapshot `doctor_prod_meetingId_2605191708092906.before_fix.bin`
//      (which contains concurrent writes to `mapping.MeetValueAttrObj`).
//   2. Connect a yjs client through y-websocket and verify it sees the initial value.
//   3. Have the yjs client overwrite `mapping.MeetValueAttrObj` once.
//   4. Send a REST `POST .../map/mapping` that overwrites `MeetValueAttrObj` again.
//   5. Assert that the yjs client now sees the value from step 4 (and not `undefined`).
//
// Prior to the fix in `libs/jwst-codec/src/doc/types/map.rs::Map::_insert`, step 4 caused
// the entire `MeetValueAttrObj` entry to disappear from the yjs client (until it
// reconnected) because the server was sending a struct whose `origin_left_id` pointed
// at a deleted, mid-chain item from the binary snapshot.

import { pathToFileURL, fileURLToPath } from 'node:url';
import path from 'node:path';
import fs from 'node:fs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..', 'homepage', 'node_modules');
const yjsMod = await import(pathToFileURL(path.join(root, 'yjs', 'dist', 'yjs.mjs')).href);
const wsMod = await import(pathToFileURL(path.join(root, 'y-websocket', 'dist', 'y-websocket.cjs')).href);
const Y = yjsMod.default ?? yjsMod;
const { WebsocketProvider } = wsMod;

const base = process.env.KECK_URL ?? 'http://127.0.0.1:3000';
const wsBase = base.replace(/^http/, 'ws');
const workspace = process.env.WS_ID ?? '2605191708092906';
const binPath = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', 'doctor_prod_meetingId_2605191708092906.before_fix.bin');

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const restDelete = async () => {
  const res = await fetch(`${base}/api/block/${workspace}`, { method: 'DELETE' });
  // 204 (deleted) and 404 (no-op) are both acceptable
  if (!res.ok && res.status !== 404) throw new Error(`delete failed: ${res.status}`);
};

const restInit = async () => {
  if (!fs.existsSync(binPath)) {
    throw new Error(`fixture file missing: ${binPath}`);
  }
  const bin = fs.readFileSync(binPath);
  const res = await fetch(`${base}/api/block/${workspace}/init`, {
    method: 'POST', headers: { 'content-type': 'application/octet-stream' }, body: bin,
  });
  if (!res.ok) throw new Error(`init failed: ${res.status}`);
};

const restGetMapping = async () => {
  const res = await fetch(`${base}/api/block/${workspace}/map/mapping`);
  if (!res.ok) throw new Error(`get mapping failed: ${res.status}`);
  return res.json();
};

const restSetMapping = async (payload) => {
  const res = await fetch(`${base}/api/block/${workspace}/map/mapping`, {
    method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(payload),
  });
  if (!res.ok) throw new Error(`set mapping failed: ${res.status}`);
};

const fail = (msg) => { console.error('verify failed:', msg); process.exit(1); };

await restDelete();
await restInit();

const doc = new Y.Doc();
const provider = new WebsocketProvider(`${wsBase}/collaboration`, workspace, doc);
const waitSynced = (p) => new Promise((r) => { if (p.synced) return r(); p.on('sync', (ok) => ok && r()); });
await waitSynced(provider);
await sleep(500);

const mapping = doc.getMap('mapping');
console.log('initial yjs MeetValueAttrObj =', JSON.stringify(mapping.get('MeetValueAttrObj')));

// Step 1: yjs client overwrites MeetValueAttrObj (this aligns server + yjs state).
doc.transact(() => {
  mapping.set('MeetValueAttrObj', { preMaxEd: 99, status: 1, fromYjs: true });
  mapping.set('updateTime', '2026-05-13 from-yjs');
});
await sleep(1500);
const afterYjs = mapping.get('MeetValueAttrObj');
console.log('after yjs write: yjs MeetValueAttrObj =', JSON.stringify(afterYjs));
if (!afterYjs || afterYjs.fromYjs !== true) {
  fail(`yjs write did not take effect, got ${JSON.stringify(afterYjs)}`);
}
const restAfterYjs = await restGetMapping();
if (restAfterYjs?.MeetValueAttrObj?.fromYjs !== true) {
  fail(`server REST GET did not reflect yjs write: ${JSON.stringify(restAfterYjs?.MeetValueAttrObj)}`);
}

// Step 2: REST POST overrides MeetValueAttrObj. This is the operation that USED TO
// silently delete the value on the yjs side.
const expected = {
  preMaxEd: 21,
  preMax222Ed: 21,
  status: 0,
  stopMaxEd: 21,
};
await restSetMapping({ MeetValueAttrObj: expected, updateTime: '2026-05-13 from-rest' });
await sleep(2500);

const finalYjs = mapping.get('MeetValueAttrObj');
const finalUpdateTime = mapping.get('updateTime');
console.log('after REST POST: yjs MeetValueAttrObj =', JSON.stringify(finalYjs));
console.log('after REST POST: yjs updateTime      =', JSON.stringify(finalUpdateTime));

provider.destroy();
doc.destroy();

if (!finalYjs || typeof finalYjs !== 'object') {
  fail(`yjs MeetValueAttrObj missing after REST POST (was: ${JSON.stringify(finalYjs)})`);
}
for (const [k, v] of Object.entries(expected)) {
  if (finalYjs[k] !== v) {
    fail(`yjs MeetValueAttrObj.${k} expected ${v}, got ${finalYjs[k]}`);
  }
}
if (finalUpdateTime !== '2026-05-13 from-rest') {
  fail(`yjs updateTime expected "2026-05-13 from-rest", got ${JSON.stringify(finalUpdateTime)}`);
}

console.log('verify ok: yjs client kept MeetValueAttrObj value after REST POST overwrite');
