import { pathToFileURL } from 'node:url';
import path from 'node:path';

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname), '..', '..', 'homepage', 'node_modules');
const yjsMod = await import(pathToFileURL(path.join(root, 'yjs', 'dist', 'yjs.mjs')).href);
const wsMod = await import(pathToFileURL(path.join(root, 'y-websocket', 'dist', 'y-websocket.cjs')).href);
const Y = yjsMod.default ?? yjsMod;
const { WebsocketProvider } = wsMod;

const base = process.env.KECK_URL ?? 'http://127.0.0.1:3000';
const wsBase = base.replace(/^http/, 'ws');
const workspace = process.env.WS_ID ?? 'doctor_prod_userName_wuyulong1';

const docA = new Y.Doc();
const docB = new Y.Doc();
const pA = new WebsocketProvider(`${wsBase}/collaboration`, workspace, docA);
const pB = new WebsocketProvider(`${wsBase}/collaboration`, workspace, docB);

const waitSynced = (provider) => new Promise((resolve) => {
  if (provider.synced) return resolve();
  provider.on('sync', (ok) => ok && resolve());
});

await Promise.all([waitSynced(pA), waitSynced(pB)]);

const mapB = docB.getMap('visit_status');
let observedUpdateTime = false;
mapB.observe((evt) => {
  if (!evt.keysChanged.has('updateTime')) return;
  observedUpdateTime = true;
  console.log('ws observed updateTime =', mapB.get('updateTime'));
});

const setMap = async (payload) => {
  const res = await fetch(`${base}/api/block/${workspace}/map/visit_status`, {
    method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(payload)
  });
  if (!res.ok) throw new Error(`set map failed: ${res.status}`);
};

console.log('set via REST: visit_status.updateTime = \"123\"');
await setMap({ updateTime: '123' });
await new Promise((r) => setTimeout(r, 2000));
const finalValue = mapB.get('updateTime');
console.log('peer yjs final visit_status.updateTime =', finalValue);

pA.destroy();
pB.destroy();
docA.destroy();
docB.destroy();

if (!observedUpdateTime || finalValue !== '123') {
  console.error('verify failed: websocket peer did not observe updateTime=123');
  process.exit(1);
}

console.log('verify ok: websocket peer observed updateTime=123');
