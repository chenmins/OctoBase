import { pathToFileURL } from 'node:url';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..', 'homepage', 'node_modules');
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
let observedVisitStatus = false;
mapB.observe((evt) => {
  if (evt.keysChanged.has('updateTime')) {
    observedUpdateTime = true;
    console.log('ws observed updateTime =', mapB.get('updateTime'));
  }
  if (evt.keysChanged.has('visit_status')) {
    observedVisitStatus = true;
    console.log('ws observed visit_status =', JSON.stringify(mapB.toJSON().visit_status));
  }
});

const getMap = async () => {
  const res = await fetch(`${base}/api/block/${workspace}/map/visit_status`);
  if (!res.ok) throw new Error(`get map failed: ${res.status}`);
  return res.json();
};

const setMap = async (payload) => {
  const res = await fetch(`${base}/api/block/${workspace}/map/visit_status`, {
    method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(payload)
  });
  if (!res.ok) throw new Error(`set map failed: ${res.status}`);
};

const assertVisitStatus = (actual, expected, label) => {
  if (!actual || typeof actual !== 'object') {
    throw new Error(`${label}: visit_status is not an object: ${JSON.stringify(actual)}`);
  }
  for (const [key, value] of Object.entries(expected)) {
    if (actual[key] !== value) {
      throw new Error(`${label}: visit_status.${key} expected ${value}, got ${actual[key]}`);
    }
  }
};

await new Promise((r) => setTimeout(r, 1000));
const restBefore = await getMap();
const yjsBefore = mapB.toJSON();
console.log('rest initial visit_status =', JSON.stringify(restBefore.visit_status));
console.log('yjs initial visit_status =', JSON.stringify(yjsBefore.visit_status));
assertVisitStatus(yjsBefore.visit_status, restBefore.visit_status, 'initial yjs sync');

const nextVisitStatus = {
  closedVisitStatus: '0',
  closedVisitId: '3472275',
  newVisitId: '3472273',
  newVisitStatus: '1',
  isVisit: '1',
  verifyTag: 'rest-ws',
};

console.log('set via REST: visit_status.updateTime = "123" and nested visit_status object');
await setMap({ updateTime: '123', visit_status: nextVisitStatus });
await new Promise((r) => setTimeout(r, 2000));
const finalJson = mapB.toJSON();
const finalValue = finalJson.updateTime;
console.log('peer yjs final visit_status.updateTime =', finalValue);
console.log('peer yjs final visit_status object =', JSON.stringify(finalJson.visit_status));

pA.destroy();
pB.destroy();
docA.destroy();
docB.destroy();

if (!observedUpdateTime || finalValue !== '123') {
  console.error('verify failed: websocket peer did not observe updateTime=123');
  process.exit(1);
}

if (!observedVisitStatus) {
  console.error('verify failed: websocket peer did not observe visit_status object update');
  process.exit(1);
}

try {
  assertVisitStatus(finalJson.visit_status, nextVisitStatus, 'final yjs sync');
} catch (err) {
  console.error('verify failed:', err.message);
  process.exit(1);
}

console.log('verify ok: websocket peer observed updateTime=123 and nested visit_status object');
