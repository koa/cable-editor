// Checks of the real backend against the database of run-realdb.sh (fresh, with local/data.sql):
// builds cables, panels, port usages and plans through GraphQL, then checks what the pages
// query (the DataLoaders), the PostGIS conversions, fitting a duct's course, owners, Schacht
// types and the refusals of the mutations. With PG_LOG (printed by run-realdb.sh) it counts the
// SQL statements per query, which shows whether the loaders batch, and the automatic sync to
// Netbox (the mock's). Changes the data, so run it once per start.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const API = process.env.API ?? 'http://127.0.0.1:8080/graphql';
const PG_LOG = process.env.PG_LOG;
const token = (await (await fetch('http://localhost:8099/realms/cable/token', { method: 'POST', body: 'code=check' })).json()).access_token;

let failures = 0;
function check(label, ok, detail = '') {
  if (!ok) failures++;
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
}

/** Runs a query; GraphQL errors are returned (for the checks of refusals), not thrown. */
async function run(query, variables = {}) {
  const response = await fetch(API, {
    method: 'POST',
    headers: { 'content-type': 'application/json', authorization: `Bearer ${token}` },
    body: JSON.stringify({ query, variables }),
  });
  return response.json();
}

/** Runs a query that must succeed, with the number of SQL statements it caused if PG_LOG is set. */
async function gql(query, variables = {}) {
  const before = PG_LOG ? fs.statSync(PG_LOG).size : 0;
  const result = await run(query, variables);
  if (result.errors) throw new Error(`${query.slice(0, 80)}: ${JSON.stringify(result.errors)}`);
  if (PG_LOG) {
    await new Promise((resolve) => setTimeout(resolve, 200));
    const added = fs.readFileSync(PG_LOG, 'utf8').slice(before);
    result.data.statements = added.split('\n').filter((line) => / LOG: {2}(statement|execute)/.test(line)).length;
  }
  return result.data;
}

/**
 * A refused request: the reason the backend sends in extensions.userError (the frontend words it,
 * see docs/fehlermeldungen.md) must have `expected`'s code and fields.
 */
async function refused(label, query, variables = {}, expected) {
  const result = await run(query, variables);
  const reason = result.errors?.[0]?.extensions?.userError;
  const matches = reason !== undefined
    && Object.entries(expected).every(([key, value]) => JSON.stringify(reason[key]) === JSON.stringify(value));
  check(label, matches, reason ? JSON.stringify(reason) : (result.errors?.[0]?.message ?? 'not refused'));
}

const statements = (data) => (data.statements === undefined ? '' : ` (${data.statements} SQL statements)`);
const near = (a, b, tolerance) => Math.abs(a - b) <= tolerance;

// ---------------------------------------------------------------- setup
const user = await gql('{ currentUser { role } }');
check('login as admin', user.currentUser.role === 'ADMIN', user.currentUser.role);

// Ducts of local/data.sql: 1 Berg 1654–Berg 925, 2 Berg 925–Berg, 3 Bühl 985–Berg,
// 4 Berg–Grosswies, 5 Berghof 1061–Grosswies, 6 Grosswies–Oberspitzwies.
// Cables ending at the panels in Berg (4) and Grosswies (6): K1 1→4, K2 4→6, K3 6→7, K4 3→4
const cables = {};
for (const [name, path] of [['K1', [1, 2]], ['K2', [4]], ['K3', [6]], ['K4', [3]]]) {
  const { createCable } = await gql('mutation($n:String!){ createCable(name:$n){ id } }', { n: name });
  cables[name] = createCable.id;
  await gql('mutation($id:Int!,$p:[Int!]){ updateCable(cableId:$id, fibers:{bundleCount:1,fiberCount:12}, path:$p){ id } }', { id: createCable.id, p: path });
}
const panelIds = {};
for (const schachtId of [4, 6]) {
  await gql('mutation($s:Int!){ createPanel(panel:{name:"Rack", schachtId:$s, children:[{name:"Kassette 1", schachtId:$s, children:[]},{name:"Kassette 2", schachtId:$s, children:[]}]}) }', { s: schachtId });
  const { schacht } = await gql('query($s:Int!){ schacht(schachtId:$s){ rootPanels { children { id name } } } }', { s: schachtId });
  panelIds[schachtId] = schacht.rootPanels[0].children.find((panel) => panel.name === 'Kassette 1').id;
  const changes = [1, 2, 3, 4, 5, 6].map((i) => ({ id: { temporary: `p${i}` }, order: i, label: `${i}`, portType: 'SPLICE' }));
  await gql('mutation($p:Int!,$c:[FlatPortInput!]!){ updatePanelPorts(panelId:$p, changes:$c, deletes:[]) }', { p: panelIds[schachtId], c: changes });
}
const ports = {};
for (const schachtId of [4, 6]) {
  const { panel } = await gql('query($p:Int!){ panel(panelId:$p){ ports { id orderNumber } } }', { p: panelIds[schachtId] });
  ports[schachtId] = panel.ports.sort((a, b) => a.orderNumber - b.orderNumber).map((port) => port.id);
}
const attach = (portId, side, cableId, fiber) => ({ portId, side, fiber: { attach: { cableId, bundle: 1, fiber } } });
const planId = async (name) => {
  await gql('mutation($n:String!){ createPlan(plan:{name:$n}) }', { n: name });
  return (await gql('{ listPlan { id name } }')).listPlan.find((plan) => plan.name === name).id;
};
// The baseline only changes by implementing a plan: K1–K2 spliced in Berg, K2–K3 in Grosswies
const initial = await planId('Erstausbau');
await gql('mutation($pl:Int!,$c:[PortUsageInput!]!){ setPortUsage(planId:$pl, changes:$c) }', {
  pl: initial,
  c: [
    ...ports[4].flatMap((id, i) => [attach(id, 'BACK', cables.K1, i + 1), attach(id, 'FRONT', cables.K2, i + 1)]),
    ...ports[6].flatMap((id, i) => [attach(id, 'BACK', cables.K2, i + 1), attach(id, 'FRONT', cables.K3, i + 1)]),
  ],
});
await gql('mutation($pl:Int!){ implementPlan(planId:$pl){ id } }', { pl: initial });
const usage = await gql('{ plan(planId:0){ usage { side } } }');
check('implementPlan fills the baseline', usage.plan.usage.length === 24, `${usage.plan.usage.length} usages`);
// An open plan: K4 instead of K1 on ports 1–3 in Berg
const rebuild = await planId('Umbau Berg');
await gql('mutation($pl:Int!,$c:[PortUsageInput!]!){ setPortUsage(planId:$pl, changes:$c) }', {
  pl: rebuild,
  c: ports[4].slice(0, 3).map((id, i) => attach(id, 'BACK', cables.K4, i + 1)),
});

// ---------------------------------------------------------------- loaders
const cableList = await gql('{ listCable { name length line { lat } path { nearSchacht { name } segments { sequence duct { id length } farSchacht { name } } farSchacht { name } } } }');
console.log(`listCable${statements(cableList)}`);
const k1 = cableList.listCable.find((cable) => cable.name === 'K1');
check('cable path', k1.path.nearSchacht.name === 'Berg 1654' && k1.path.segments.map((s) => s.duct.id).join() === '1,2' && k1.path.farSchacht.name === 'Berg',
  `${k1.path.nearSchacht.name} ${k1.path.segments.map((s) => s.duct.id)} ${k1.path.farSchacht.name}`);
for (const cable of cableList.listCable) {
  const sum = cable.path.segments.reduce((total, segment) => total + segment.duct.length, 0);
  check(`length of ${cable.name} = its ducts`, near(cable.length, sum, 0.01), `${cable.length.toFixed(2)} / ${sum.toFixed(2)}`);
}
check('cable line', k1.line.length > 2, `${k1.line.length} points`);

const ductList = await gql('{ listDuct { id length line { lat } schachtA { name typ { name } position { e } location { lat } } schachtZ { name } cables { name } } }');
console.log(`listDuct${statements(ductList)}`);
const ductCables = Object.fromEntries(ductList.listDuct.map((duct) => [duct.id, duct.cables.map((cable) => cable.name).join()]));
check('cables per duct', JSON.stringify(ductCables) === JSON.stringify({ 1: 'K1', 2: 'K1', 3: 'K4', 4: 'K2', 5: '', 6: 'K3' }), JSON.stringify(ductCables));
check('Schacht type and positions', ductList.listDuct.every((duct) => duct.schachtA.typ && duct.schachtA.position && duct.schachtA.location));

const schachtList = await gql('{ listSchacht { id name typ { name } rootPanels { name } connectingDuct { duct { id } schacht { name } } } }');
console.log(`listSchacht${statements(schachtList)}`);
const berg = schachtList.listSchacht.find((schacht) => schacht.id === 4);
check('connecting ducts', berg.connectingDuct.map((c) => `${c.duct.id}:${c.schacht.name}`).sort().join() === '2:Berg 925,3:Bühl 985,4:Grosswies',
  berg.connectingDuct.map((c) => `${c.duct.id}:${c.schacht.name}`).join());
check('root panels', berg.rootPanels.map((panel) => panel.name).join() === 'Rack');

const overview = await gql('query($pl:Int!){ schacht(schachtId:4) { cables { cable { name } path { farSchacht { name } } fibers { fiber usedPort(planId:$pl) { port { label panel { name } } modifiedInPlan } otherEnd { cable { schacht { name } } } } } } }', { pl: rebuild });
console.log(`Schacht overview${statements(overview)}`);
const used = Object.fromEntries(overview.schacht.cables.map((end) => [end.cable.name,
  end.fibers.filter((fiber) => fiber.usedPort).map((fiber) => `${fiber.fiber}${fiber.usedPort.modifiedInPlan ? '*' : ''}`).join()]));
check('used ports in the plan', JSON.stringify(used) === JSON.stringify({ K1: '4,5,6', K2: '1,2,3,4,5,6', K4: '1*,2*,3*' }), JSON.stringify(used));
const farEnds = overview.schacht.cables.map((end) => `${end.cable.name}:${end.path.farSchacht.name}`).sort().join();
check('cable ends seen from the Schacht', farEnds === 'K1:Berg 1654,K2:Grosswies,K4:Bühl 985', farEnds);

const planned = await gql('query($pl:Int!){ plan(planId:$pl) { rootPanels { panel { schacht { id } } allChildrenRecursive { ports { label usage(side:BACK) { fiber { cable { name } } modifiedInPlan cableSideEndPort(planId:$pl) { port { label } } } currentUsage(side:BACK) { fiber { cable { name } } } } } } } }', { pl: rebuild });
console.log(`plan panels${statements(planned)}`);
const bergPorts = planned.plan.rootPanels.find((root) => root.panel.schacht.id === 4).allChildrenRecursive.flatMap((panel) => panel.ports);
const described = bergPorts.map((port) => `${port.label}:${port.usage?.fiber.cable.name}${port.usage?.modifiedInPlan ? '*' : ''}/${port.currentUsage?.fiber.cable.name}`).join();
check('plan usage against the baseline', described === '1:K4*/K1,2:K4*/K1,3:K4*/K1,4:K1/K1,5:K1/K1,6:K1/K1', described);

const end = await gql('query($c:Int!){ cable(cableId:$c) { end(schachtId:6) { usedPorts(planId:0) { side } fibers { fiber usedPort(planId:0) { otherSide { fiber { cable { name } } } } otherEnd { cable { schacht { name } } } } } } }', { c: cables.K2 });
console.log(`cable end${statements(end)}`);
const spliced = end.cable.end.fibers.filter((fiber) => fiber.usedPort).map((fiber) => fiber.usedPort.otherSide.fiber.cable.name);
check('other side of the ports', spliced.length === 6 && spliced.every((name) => name === 'K3'), spliced.join());
check('other end of the fibers', end.cable.end.fibers.every((fiber) => fiber.otherEnd.cable.schacht.name === 'Berg'));

// ---------------------------------------------------------------- PostGIS
// Bern, old observatory: LV95 2600000/1200000 = 46.9510828 N, 7.4386324 E (swisstopo)
const bern = await gql('{ a: convertPoint(position:{lv95:{e:2600000,n:1200000}}) { wgs84 { lat lng } } b: convertPoint(position:{wgs84:{lat:46.9510828,lng:7.4386324}}) { lv95 { e n } } }');
check('LV95 → WGS84', near(bern.a.wgs84.lat, 46.9510828, 1e-6) && near(bern.a.wgs84.lng, 7.4386324, 1e-6), JSON.stringify(bern.a.wgs84));
check('WGS84 → LV95', near(bern.b.lv95.e, 2600000, 0.1) && near(bern.b.lv95.n, 1200000, 0.1), JSON.stringify(bern.b.lv95));
await refused('position outside of Switzerland', '{ convertPoint(position:{wgs84:{lat:48.2082,lng:16.3738}}) { lv95 { e } } }', {}, { code: 'PositionOutsideSwitzerland' });

// Duct 5: Berghof 1061 (A) to Grosswies (Z), its stored points between them
const A = [2709099.229648728, 1252890.2705348001];
const Z = [2709065.6546643884, 1252917.1140004809];
const between = [[2709096.0601824243, 1252890.7653514901], [2709069.806984851, 1252913.1799495178]];
const line = (system, points, dx = 0, dy = 0) => ({ system, points: points.map(([x, y]) => ({ x: x - dx, y: y - dy })) });
const checkLine = async (l) => (await gql('query($l:LineInput!){ checkDuctLine(schachtA:5, schachtZ:6, line:$l) { reversed removedEnds endDistance length needsConfirmation } }', { l })).checkDuctLine;
const reversedPoints = [Z, ...between.slice().reverse(), A];
for (const [system, dx, dy] of [['LV95', 0, 0], ['LV03', 2000000, 1000000]]) {
  const result = await checkLine(line(system, reversedPoints, dx, dy));
  check(`${system} line from Z to A`, result.reversed && result.removedEnds === 2 && !result.needsConfirmation && near(result.length, 43.45, 0.05), JSON.stringify(result));
}
const wgs84 = [];
for (const [e, n] of [A, ...between, Z]) {
  wgs84.push((await gql('query($e:Float!,$n:Float!){ convertPoint(position:{lv95:{e:$e,n:$n}}) { wgs84 { lat lng } } }', { e, n })).convertPoint.wgs84);
}
const wgsResult = await checkLine({ system: 'WGS84', points: wgs84.map((p) => ({ x: p.lng, y: p.lat })) });
check('WGS84 line from A to Z', !wgsResult.reversed && wgsResult.removedEnds === 2 && near(wgsResult.length, 43.45, 0.05), JSON.stringify(wgsResult));
const far = line('LV95', [A, ...between, [Z[0] - 20, Z[1]]]);
const farResult = await checkLine(far);
check('end 20 m off its Schacht', farResult.needsConfirmation && near(farResult.endDistance, 20, 0.01), JSON.stringify(farResult));
await refused('storing it unconfirmed', 'mutation($l:LineInput){ setDuctLine(ductId:5, line:$l) { id } }', { l: far }, { code: 'LineNeedsConfirmation' });
const stored = await gql('mutation($l:LineInput){ setDuctLine(ductId:5, line:$l, confirmed:true) { length } }', { l: far });
check('storing it confirmed', near(stored.setDuctLine.length, farResult.length, 0.01));
await gql('mutation($l:LineInput){ setDuctLine(ductId:5, line:$l) { id } }', { l: line('LV95', reversedPoints) });

// ---------------------------------------------------------------- mutations
const created = await gql('mutation{ createSchacht(schacht:{name:"Neu", typeId:0, position:{wgs84:{lat:47.4185,lng:8.885}}, ownerId:1, lagebestimmung:GENAU}) { id location { lat lng } owner { id } lagebestimmung } }');
const schachtId = created.createSchacht.id;
check('createSchacht at its position', near(created.createSchacht.location.lat, 47.4185, 1e-6));
check('createSchacht with owner and Lagebestimmung', created.createSchacht.owner.id === 1 && created.createSchacht.lagebestimmung === 'GENAU', JSON.stringify(created.createSchacht));
await refused('createSchacht with an unknown owner', 'mutation{ createSchacht(schacht:{name:"X", ownerId:999, lagebestimmung:UNGENAU}) { id } }', {}, { code: 'NotFound', kind: 'Owner', id: 999 });
const duct = await gql('mutation($z:Int!){ createDuct(duct:{schachtA:6, schachtZ:$z, ownerId:1, leitungskataster:false, lagebestimmung:UNGENAU}) { id length line { lat } } }', { z: schachtId });
check('createDuct without course: straight', duct.createDuct.line.length === 2 && duct.createDuct.length > 0);
const schachtChanged = async () => (await gql('query($s:Int!){ schacht(schachtId:$s) { changedAt } }', { s: schachtId })).schacht.changedAt;
const ductChanged = async () => (await gql('query($d:Int!){ duct(ductId:$d) { changedAt } }', { d: duct.createDuct.id })).duct.changedAt;
const [schachtBefore, ductBefore6] = [await schachtChanged(), await ductChanged()];
await gql('mutation($id:Int!){ updateSchacht(schachtId:$id, schacht:{name:"Neu", typeId:0, position:{wgs84:{lat:47.4185,lng:8.885}}, ownerId:1, lagebestimmung:UNGENAU}) { id } }', { id: schachtId });
check('a new Lagebestimmung changes the Schacht, not its ducts', (await schachtChanged()) !== schachtBefore && (await ductChanged()) === ductBefore6);
await gql('mutation($id:Int!){ updateSchacht(schachtId:$id, schacht:{name:"Neu", typeId:0, position:{lv95:{e:2709150,n:1253000}}, ownerId:1, lagebestimmung:UNGENAU}) { id } }', { id: schachtId });
const moved = await gql('query($d:Int!){ duct(ductId:$d) { length } }', { d: duct.createDuct.id });
check('the duct follows its Schacht', !near(moved.duct.length, duct.createDuct.length, 1), `${duct.createDuct.length.toFixed(1)} → ${moved.duct.length.toFixed(1)}`);
// What the Leitungskataster takes from a duct
const updateDuct = 'mutation($id:Int!,$d:DuctInput!){ updateDuct(ductId:$id, duct:$d) { owner { id } leitungskataster lagebestimmung widthMm changedAt } }';
const ductInput = (changes) => ({ id: duct.createDuct.id, d: { schachtA: 6, schachtZ: schachtId, ownerId: 1, leitungskataster: false, lagebestimmung: 'UNGENAU', ...changes } });
const ductBefore = (await gql('query($d:Int!){ duct(ductId:$d) { changedAt } }', { d: duct.createDuct.id })).duct.changedAt;
const delivered = (await gql(updateDuct, ductInput({ leitungskataster: true, lagebestimmung: 'GENAU', widthMm: 300 }))).updateDuct;
check('updateDuct stores the delivery', delivered.leitungskataster && delivered.lagebestimmung === 'GENAU' && delivered.widthMm === 300, JSON.stringify(delivered));
check('updateDuct with a change moves changedAt', delivered.changedAt !== ductBefore);
const unchanged = (await gql(updateDuct, ductInput({ leitungskataster: true, lagebestimmung: 'GENAU', widthMm: 300 }))).updateDuct;
check('updateDuct without change keeps changedAt', unchanged.changedAt === delivered.changedAt);
check('the owner counts the delivered duct', (await gql('{ listOwner { id deliveredDuctCount } }')).listOwner.find((o) => o.id === 1).deliveredDuctCount === 1);
await refused('updateDuct with a width over 4 m', updateDuct, ductInput({ widthMm: 4001 }), { code: 'WidthOutOfRange', max: 4000 });
await refused('updateDuct with an unknown owner', updateDuct, ductInput({ ownerId: 999 }), { code: 'NotFound', kind: 'Owner', id: 999 });
await refused('deleteDuct with cables', 'mutation{ deleteDuct(ductId:4) }', {}, { code: 'DuctHasCables' });
await refused('deleteSchacht with ducts', 'mutation($id:Int!){ deleteSchacht(schachtId:$id) }', { id: schachtId }, { code: 'SchachtReferenced', panels: 0, ducts: 1 });
await refused('updateCable to an empty path', 'mutation($c:Int!){ updateCable(cableId:$c, path:[]) { id } }', { c: cables.K4 }, { code: 'CableWithoutSegment' });
await refused('setPortUsage on the baseline', 'mutation($p:Int!){ setPortUsage(planId:0, changes:[{portId:$p, side:FRONT, fiber:{remove:true}}]) }', { p: ports[4][0] }, { code: 'BaselineUnchangeable' });
await refused('deleteCable attached in the baseline', 'mutation($c:Int!){ deleteCable(cableId:$c) }', { c: cables.K1 }, { code: 'CableAttached', plans: [{ plan: 'Baseline', ports: 6 }] });
await refused('deleteCable attached in a plan', 'mutation($c:Int!){ deleteCable(cableId:$c) }', { c: cables.K4 }, { code: 'CableAttached', plans: [{ plan: 'Umbau Berg', ports: 3 }] });
const spare = (await gql('mutation{ createCable(name:"Reserve"){ id } }')).createCable.id;
await gql('mutation($c:Int!){ updateCable(cableId:$c, path:[5]){ id } }', { c: spare });
check('deleteCable without ports', (await gql('mutation($c:Int!){ deleteCable(cableId:$c) }', { c: spare })).deleteCable === true);
await gql('mutation($d:Int!){ deleteDuct(ductId:$d) }', { d: duct.createDuct.id });
await gql('mutation($id:Int!){ deleteSchacht(schachtId:$id) }', { id: schachtId });

// ---------------------------------------------------------------- owners
const OWNER = 'id name lkName isDefault schachtCount ductCount deliveredDuctCount';
const owners = await gql(`{ listOwner { ${OWNER} } }`);
console.log(`listOwner${statements(owners)}`);
const standard = owners.listOwner.find((owner) => owner.isDefault);
check('the default owner has all Schächte and ducts', standard?.schachtCount === 7 && standard?.ductCount === 6 && standard?.deliveredDuctCount === 0, JSON.stringify(standard));
const createOwner = 'mutation($o:OwnerInput!){ createOwner(owner:$o) { id name lkName isDefault } }';
const other = (await gql(createOwner, { o: { name: '  Private Leitung ', lkName: 'Keine_Angabe' } })).createOwner;
check('createOwner trims', other.name === 'Private Leitung' && other.lkName === 'Keine_Angabe' && !other.isDefault, JSON.stringify(other));
await refused('createOwner without name', createOwner, { o: { name: ' ' } }, { code: 'NameMissing' });
await refused('createOwner with a taken name', createOwner, { o: { name: 'Private Leitung' } }, { code: 'NameTaken', kind: 'Owner', name: 'Private Leitung' });
await refused('createOwner with a long name in the delivery', createOwner, { o: { name: 'X', lkName: 'x'.repeat(81) } }, { code: 'LkNameTooLong', max: 80 });
const updateOwner = 'mutation($id:Int!,$o:OwnerInput!){ updateOwner(ownerId:$id, owner:$o) { name lkName } }';
const cleared = (await gql(updateOwner, { id: other.id, o: { name: 'Private Leitung', lkName: '' } })).updateOwner;
check('updateOwner clears empty values', cleared.lkName === null, JSON.stringify(cleared));
check('updateOwner keeps its own name', (await gql(updateOwner, { id: other.id, o: { name: 'Private Leitung' } })).updateOwner.name === 'Private Leitung');
// Name and name in the delivery of the owner are the Eigentuemer of its objects
const changedAt = async () => (await gql('{ duct(ductId:1) { changedAt } }')).duct.changedAt;
const before = await changedAt();
await gql(updateOwner, { id: standard.id, o: { name: standard.name, lkName: standard.lkName } });
check('updateOwner without change keeps changedAt of its ducts', (await changedAt()) === before);
await gql(updateOwner, { id: standard.id, o: { name: standard.name, lkName: 'Genossenschaft' } });
const afterName = await changedAt();
check('updateOwner with a new name in the delivery changes its ducts', afterName !== before, `${before} → ${afterName}`);
await gql(updateOwner, { id: standard.id, o: { name: standard.name } });
const restored = await changedAt();
const setDefault = 'mutation($id:Int!){ setDefaultOwner(ownerId:$id) { id isDefault } }';
check('setDefaultOwner', (await gql(setDefault, { id: other.id })).setDefaultOwner.isDefault);
const defaults = (await gql(`{ listOwner { ${OWNER} } }`)).listOwner.filter((owner) => owner.isDefault);
check('exactly one default owner', defaults.length === 1 && defaults[0].id === other.id, JSON.stringify(defaults));
check('setDefaultOwner keeps changedAt', (await changedAt()) === restored);
const deleteOwner = 'mutation($id:Int!){ deleteOwner(ownerId:$id) }';
await refused('deleteOwner of the default owner', deleteOwner, { id: other.id }, { code: 'DefaultOwnerNotDeletable' });
await refused('deleteOwner with Schächte and ducts', deleteOwner, { id: standard.id }, { code: 'OwnerReferenced', schaechte: 7, ducts: 6 });
await gql(setDefault, { id: standard.id });
check('deleteOwner without objects', (await gql(deleteOwner, { id: other.id })).deleteOwner === true);
await refused('updateOwner of a deleted owner', updateOwner, { id: other.id, o: { name: 'Weg' } }, { code: 'NotFound', kind: 'Owner', id: other.id });

// ---------------------------------------------------------------- Schacht types
const TYP = 'id name icon lkmapObjektart dimension1Mm dimension2Mm schachtCount';
const types = await gql(`{ listSchachtTyp { ${TYP} } }`);
console.log(`listSchachtTyp${statements(types)}`);
const usedTyp = types.listSchachtTyp.find((typ) => typ.schachtCount > 0);
check('a type with Schächte', usedTyp !== undefined, JSON.stringify(types.listSchachtTyp.map(({ icon, ...typ }) => typ)));
const createTyp = 'mutation($t:SchachtTypInput!){ createSchachtTyp(typ:$t) { id name icon lkmapObjektart dimension1Mm dimension2Mm } }';
const newTyp = (await gql(createTyp, { t: { name: ' Rechteckschacht ', lkmapObjektart: 'SCHACHT_RECHTECKIG', dimension1Mm: 1200, dimension2Mm: 800 } })).createSchachtTyp;
check('createSchachtTyp trims and gets an icon', newTyp.name === 'Rechteckschacht' && newTyp.icon.startsWith('<svg') && newTyp.dimension2Mm === 800, JSON.stringify(newTyp));
const typInput = (changes) => ({ t: { name: 'X', lkmapObjektart: 'SCHACHT_RUND', ...changes } });
await refused('createSchachtTyp without name', createTyp, typInput({ name: ' ' }), { code: 'NameMissing' });
await refused('createSchachtTyp with a long name', createTyp, typInput({ name: 'x'.repeat(21) }), { code: 'NameTooLong', max: 20 });
await refused('createSchachtTyp with a taken name', createTyp, typInput({ name: 'Rechteckschacht' }), { code: 'NameTaken', kind: 'SchachtTyp', name: 'Rechteckschacht' });
await refused('createSchachtTyp with dimension 2 alone', createTyp, typInput({ dimension2Mm: 500 }), { code: 'Dimension2WithoutDimension1' });
await refused('createSchachtTyp with dimension 2 larger', createTyp, typInput({ dimension1Mm: 500, dimension2Mm: 600 }), { code: 'DimensionsSwapped' });
await refused('createSchachtTyp with a dimension over 4 m', createTyp, typInput({ dimension1Mm: 4001 }), { code: 'DimensionOutOfRange', dimension: 1, max: 4000 });
await refused('createSchachtTyp with an icon that is no SVG', createTyp, typInput({ icon: '<html><svg/></html>' }), { code: 'IconNotSvg' });
await refused('createSchachtTyp with a malformed icon', createTyp, typInput({ icon: '<svg><g></svg>' }), { code: 'IconNotSvg' });
check('createSchachtTyp with an icon behind an XML declaration', (await gql(createTyp, typInput({ name: 'Mit Icon', icon: '<?xml version="1.0"?>\n<svg xmlns="http://www.w3.org/2000/svg"/>' }))).createSchachtTyp.icon.includes('<svg'));
const updateTyp = 'mutation($id:Int!,$t:SchachtTypInput!){ updateSchachtTyp(typId:$id, typ:$t) { name icon lkmapObjektart dimension1Mm } }';
check('updateSchachtTyp keeps its own name', (await gql(updateTyp, { id: newTyp.id, t: { name: 'Rechteckschacht', lkmapObjektart: 'SCHACHT_RECHTECKIG' } })).updateSchachtTyp.dimension1Mm === null);
// The Objektart and dimensions of the type are those of its Schächte
const usedSchacht = (await gql('{ listSchacht { id changedAt typ { id } } }')).listSchacht.find((schacht) => schacht.typ?.id === usedTyp.id);
const schachtChangedAt = async () => (await gql('query($s:Int!){ schacht(schachtId:$s) { changedAt } }', { s: usedSchacht.id })).schacht.changedAt;
const renamed = (await gql(updateTyp, { id: usedTyp.id, t: { name: `${usedTyp.name} neu`, lkmapObjektart: usedTyp.lkmapObjektart } })).updateSchachtTyp;
check('updateSchachtTyp without icon keeps it', renamed.icon === usedTyp.icon);
check('renaming a type keeps changedAt of its Schächte', (await schachtChangedAt()) === usedSchacht.changedAt);
await gql(updateTyp, { id: usedTyp.id, t: { name: usedTyp.name, lkmapObjektart: 'BAUWERK' } });
check('a new Objektart changes its Schächte', (await schachtChangedAt()) !== usedSchacht.changedAt);
const deleteTyp = 'mutation($id:Int!){ deleteSchachtTyp(typId:$id) }';
await refused('deleteSchachtTyp with Schächte', deleteTyp, { id: usedTyp.id }, { code: 'SchachtTypReferenced', schaechte: usedTyp.schachtCount });
check('deleteSchachtTyp without Schächte', (await gql(deleteTyp, { id: newTyp.id })).deleteSchachtTyp === true);
check('schachtTyp of a deleted type', (await gql('query($id:Int!){ schachtTyp(typId:$id) { id } }', { id: newTyp.id })).schachtTyp === null);

// ---------------------------------------------------------------- Netbox sync
// The backend's worker syncs the plan active in Netbox after every change that affects it
// (docs/netbox-sync.md); run-realdb.sh points it at the mock's Netbox without circuits, which
// counts the queries: the plans here have no connectors, so a run only lists the circuits.
const netboxSync = async () => (await gql('{ netboxSync { state pending lastRun retryAt activePlan { id } issues { __typename } error { message } } }')).netboxSync;
const netboxQueries = async () => (await (await fetch('http://localhost:8099/netbox/queries')).json()).count;
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
/** The state once a run after `lastRun` is done (at most 20 s), else the last one */
async function nextRun(lastRun) {
  let sync;
  for (let i = 0; i < 40; i++) {
    sync = await netboxSync();
    if (sync.lastRun !== lastRun && !sync.pending) return sync;
    await sleep(500);
  }
  return sync;
}
let sync = await nextRun(null);
check('the sync ran after the changes above', sync.state === 'SYNCHRON' && sync.issues.length === 0 && sync.error === null, JSON.stringify(sync));
check('the baseline is active in Netbox', sync.activePlan?.id === 0);
const netboxPlan = await planId('Netbox-Test');
let queries = await netboxQueries();
await gql('mutation($pl:Int!,$c:[PortUsageInput!]!){ setPortUsage(planId:$pl, changes:$c) }', { pl: netboxPlan, c: [attach(ports[4][4], 'BACK', cables.K4, 5)] });
await gql('mutation($pl:Int!){ updatePlan(planId:$pl, name:"Netbox-Test") { id } }', { pl: netboxPlan });
await sleep(2000);
sync = await netboxSync();
check('changing a plan not active in Netbox starts no sync', !sync.pending && (await netboxQueries()) === queries, JSON.stringify(sync));
const activated = await gql('mutation($pl:Int!){ setNetboxActivePlan(planId:$pl) { id netboxActive } }', { pl: netboxPlan });
sync = await nextRun(sync.lastRun);
const active = (await gql('{ listPlan { id netboxActive } }')).listPlan.filter((plan) => plan.netboxActive).map((plan) => plan.id);
check('setNetboxActivePlan makes only this plan active', activated.setNetboxActivePlan.netboxActive && JSON.stringify(active) === JSON.stringify([netboxPlan]), JSON.stringify(active));
check('activating a plan syncs it', sync.activePlan?.id === netboxPlan && (await netboxQueries()) > queries, JSON.stringify(sync));
queries = await netboxQueries();
await gql('mutation($pl:Int!,$c:[PortUsageInput!]!){ setPortUsage(planId:$pl, changes:$c) }', { pl: netboxPlan, c: [attach(ports[4][5], 'BACK', cables.K4, 6)] });
sync = await nextRun(sync.lastRun);
check('changing the active plan syncs it', (await netboxQueries()) > queries, JSON.stringify(sync));
queries = await netboxQueries();
await gql('mutation($id:Int!){ updateCable(cableId:$id, fibers:{bundleCount:1,fiberCount:24}){ id } }', { id: cables.K4 });
sync = await nextRun(sync.lastRun);
check('changing a cable syncs', (await netboxQueries()) > queries, JSON.stringify(sync));
await gql('mutation($pl:Int!){ implementPlan(planId:$pl){ id } }', { pl: netboxPlan });
sync = await nextRun(sync.lastRun);
check('implementing the active plan makes the baseline active', sync.activePlan?.id === 0, JSON.stringify(sync));
queries = await netboxQueries();
await gql('mutation{ syncNetbox }');
sync = await nextRun(sync.lastRun);
check('syncNetbox syncs right away', sync.state === 'SYNCHRON' && (await netboxQueries()) > queries, JSON.stringify(sync));
await refused('setNetboxActivePlan of a missing plan', 'mutation{ setNetboxActivePlan(planId:999999) { id } }', {}, { code: 'NotFound', kind: 'Plan', id: 999999 });

// ---------------------------------------------------------------- Leitungskataster
// run-realdb.sh configures lkmap (Datenlieferant CHE-123.456.789, Datenherr CHE-987.654.321, prefix
// ch4711ab): one delivery for the whole network, named after the Datenherr
const lkmapExport = 'query{ lkmapExport { datenherr lkmap { fileName xtf } perimeter { fileName xtf } perimeterArea { lat lng } checksum schaechte { id } ducts { id } schaechteWithoutPosition { id } ductsWithoutLine { id } deliveries { id } firstChangeSinceDelivery } }';
const downloadLkmap = 'mutation{ downloadLkmap { delivery { id createdBy schachtCount ductCount checksum deliveredAt } files { fileName content } } }';
const setDelivered = 'mutation($d:Int!, $v:Boolean!){ setLkmapDelivered(deliveryId:$d, delivered:$v) { id deliveredAt } }';
const empty = (await gql(lkmapExport)).lkmapExport;
check('lkmapExport with nothing delivered delivers nothing', empty.datenherr === 'CHE-987.654.321' && empty.lkmap === null && empty.checksum === null && empty.ducts.length === 0, JSON.stringify(empty));
await refused('downloadLkmap with nothing delivered', downloadLkmap, {}, { code: 'NothingToDeliver' });
const deliveredDuct = (a, z) => ({ schachtA: a, schachtZ: z, ownerId: standard.id, leitungskataster: true, lagebestimmung: 'GENAU' });
// Duct 5 (Berghof 1061–Grosswies) with its course, a straight one Berg 1654–Berg, one from a Schacht without position
await gql('mutation($d:DuctInput!){ updateDuct(ductId:5, duct:$d) { id } }', { d: deliveredDuct(5, 6) });
const straight = (await gql('mutation($d:DuctInput!){ createDuct(duct:$d) { id } }', { d: deliveredDuct(1, 4) })).createDuct.id;
const unlocated = (await gql('mutation{ createSchacht(schacht:{name:"Ohne Position", ownerId:1, lagebestimmung:UNGENAU}) { id } }')).createSchacht.id;
const unlocatedDuct = (await gql('mutation($d:DuctInput!){ createDuct(duct:$d) { id } }', { d: deliveredDuct(unlocated, 6) })).createDuct.id;
const exported = await gql(lkmapExport);
const lk = exported.lkmapExport;
console.log(`lkmapExport${statements(exported)}`);
check('lkmapExport file names', lk.lkmap.fileName === 'che-987-654-321-kommunikation-lkmap.xtf'
  && lk.perimeter.fileName === 'che-987-654-321-zustaendigkeit-peri.xtf', `${lk.lkmap.fileName}, ${lk.perimeter.fileName}`);
check('lkmapExport names Datenherr and Datenlieferant', lk.lkmap.xtf.includes('<Datenherr>CHE-987.654.321</Datenherr>') && lk.lkmap.xtf.includes('<Datenlieferant>CHE-123.456.789</Datenlieferant>')
  && lk.perimeter.xtf.includes('<Datenherr>CHE-987.654.321</Datenherr>'));
check('lkmapExport delivers the located ducts and their Schächte', lk.ducts.length === 2 && lk.schaechte.length === 4, `${lk.ducts.length} ducts, ${lk.schaechte.length} Schächte`);
check('lkmapExport reports what lacks a position', JSON.stringify(lk.schaechteWithoutPosition) === JSON.stringify([{ id: unlocated }])
  && JSON.stringify(lk.ductsWithoutLine) === JSON.stringify([{ id: unlocatedDuct }]), JSON.stringify([lk.schaechteWithoutPosition, lk.ductsWithoutLine]));
check('lkmapExport has the perimeter in WGS84', lk.perimeterArea.length > 4 && lk.perimeterArea.every((p) => near(p.lat, 47.3, 0.5) && near(p.lng, 8.7, 0.8)), JSON.stringify(lk.perimeterArea[0]));
check('lkmapExport has a checksum and no deliveries yet', /^[0-9a-f]{64}$/.test(lk.checksum) && lk.deliveries.length === 0 && lk.firstChangeSinceDelivery === null);
// The download: the ZIPs of the same name with the file, logged once per state of the data
const download = (await gql(downloadLkmap)).downloadLkmap;
const zipEntry = (base64) => {
  const zip = Buffer.from(base64, 'base64');
  // local file header: signature, the name's length at 26, the name at 30
  return zip.readUInt32LE(0) === 0x04034b50 ? zip.toString('utf8', 30, 30 + zip.readUInt16LE(26)) : null;
};
check('downloadLkmap delivers the ZIPs', JSON.stringify(download.files.map((f) => [f.fileName, zipEntry(f.content)])) === JSON.stringify([
  ['che-987-654-321-kommunikation-lkmap.zip', 'che-987-654-321-kommunikation-lkmap.xtf'],
  ['che-987-654-321-zustaendigkeit-peri.zip', 'che-987-654-321-zustaendigkeit-peri.xtf']]), JSON.stringify(download.files.map((f) => f.fileName)));
check('downloadLkmap logs the delivery', download.delivery.checksum === lk.checksum && download.delivery.createdBy === 'tester'
  && download.delivery.ductCount === 2 && download.delivery.schachtCount === 4 && download.delivery.deliveredAt === null, JSON.stringify(download.delivery));
const again = (await gql(downloadLkmap)).downloadLkmap;
check('downloading the same data again keeps the unmarked delivery', again.delivery.id === download.delivery.id);
const marked = (await gql(setDelivered, { d: download.delivery.id, v: true })).setLkmapDelivered;
check('setLkmapDelivered marks it', marked.deliveredAt !== null);
const markedAgain = (await gql(setDelivered, { d: download.delivery.id, v: true })).setLkmapDelivered;
check('marking it again keeps the time', markedAgain.deliveredAt === marked.deliveredAt);
check('unchanged after the delivery', (await gql(lkmapExport)).lkmapExport.firstChangeSinceDelivery === null);
await gql('mutation($d:DuctInput!){ updateDuct(ductId:5, duct:$d) { id } }', { d: { ...deliveredDuct(5, 6), widthMm: 300 } });
const changed = (await gql(lkmapExport)).lkmapExport;
check('a change after the delivery shows', changed.checksum !== lk.checksum && changed.firstChangeSinceDelivery !== null, JSON.stringify([changed.checksum, changed.firstChangeSinceDelivery]));
const next = (await gql(downloadLkmap)).downloadLkmap;
check('downloading changed data logs a new delivery', next.delivery.id !== download.delivery.id && next.delivery.checksum === changed.checksum);
check('setLkmapDelivered takes the mark back', (await gql(setDelivered, { d: download.delivery.id, v: false })).setLkmapDelivered.deliveredAt === null);
await refused('setLkmapDelivered of a missing delivery', setDelivered, { d: 999999, v: true }, { code: 'NotFound', kind: 'LkmapDelivery', id: 999999 });
const lagebestimmung = (oid) => lk.lkmap.xtf.slice(lk.lkmap.xtf.indexOf(`TID="${oid}"`)).match(/<Lagebestimmung>(\w+)</)?.[1];
check('a duct with course keeps its Lagebestimmung', lagebestimmung('ch4711abt0000005') === 'genau');
check('a straight duct is unbekannt', lagebestimmung(`ch4711abt${String(straight).padStart(7, '0')}`) === 'unbekannt');
// The perimeter: a closed ring around every delivered coordinate (the Checkservice warns about
// objects outside of it; the bounding box of the ring is a rough check)
const coords = (xtf) => [...xtf.matchAll(/<C1>([\d.]+)<\/C1>\s*<C2>([\d.]+)<\/C2>/g)].map((m) => [Number(m[1]), Number(m[2])]);
const ring = coords(lk.perimeter.xtf);
const [minE, maxE, minN, maxN] = [Math.min(...ring.map((c) => c[0])), Math.max(...ring.map((c) => c[0])), Math.min(...ring.map((c) => c[1])), Math.max(...ring.map((c) => c[1]))];
check('the perimeter is a closed ring', ring.length > 4 && JSON.stringify(ring[0]) === JSON.stringify(ring.at(-1)), `${ring.length} points`);
check('the perimeter lies around the delivered objects', coords(lk.lkmap.xtf).every(([e, n]) => e > minE + 9 && e < maxE - 9 && n > minN + 9 && n < maxN - 9));
// ilivalidator against the models of the SIA, like the canton's Checkservice (needs Java or nix
// and the network; SKIP_ILIVALIDATOR=1 leaves it out)
if (process.env.SKIP_ILIVALIDATOR) {
  console.log('skip ilivalidator');
} else {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'lkmap-'));
  const files = [lk.lkmap, lk.perimeter].map((transfer) => {
    const file = path.join(dir, transfer.fileName);
    fs.writeFileSync(file, transfer.xtf);
    return file;
  });
  const validator = path.join(path.dirname(new URL(import.meta.url).pathname), '../lkmap/validate.sh');
  const run = spawnSync(validator, files, { encoding: 'utf8' });
  const output = `${run.stdout}${run.stderr}`;
  check('ilivalidator accepts both files', run.status === 0 && output.includes('...validation done'),
    output.split('\n').filter((line) => /^Error|failed/.test(line)).join('; ') || `exit ${run.status}`);
}

console.log(failures ? `\n${failures} Prüfung(en) fehlgeschlagen` : '\nAlle Prüfungen bestanden');
process.exit(failures ? 1 : 0);
