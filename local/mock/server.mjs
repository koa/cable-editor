// Mock backend to try the frontend without Postgres/PostGIS, Keycloak or the Rust backend, e.g.
// for layout checks with Playwright (see screenshot.mjs). It serves on one origin:
// - the trunk build (cable-editor-frontend/dist) with SPA fallback,
// - a fake OIDC provider that logs in everyone without a form (RS256 ID token, PKCE unchecked),
// - both GraphQL schemas (generated into cable-editor-frontend/graphql by the frontend build)
//   over a small in-memory data set; mutations succeed without changing it.
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { buildSchema, graphql, GraphQLError } from 'graphql';
import { generateKeyPair, exportJWK, SignJWT } from 'jose';
import { createHash } from 'node:crypto';

const FRONTEND = path.join(path.dirname(fileURLToPath(import.meta.url)), '../../cable-editor-frontend');
const PORT = Number(process.env.MOCK_PORT ?? 8099);
// Role of the mock user (READER, PLANNER or ADMIN), to check what the frontend hides; DENIED
// refuses the login like a provider that doesn't allow the user's groups
const ROLE = process.env.MOCK_ROLE ?? 'ADMIN';
// Result of the last Netbox sync (docs/netbox-sync.md): OK (default), ISSUES, FEHLER or none
// (NEU, no run yet)
const NETBOX = process.env.MOCK_NETBOX ?? 'OK';
// Mutations failing with an unexpected server error (extensions.origin) like a broken database,
// separated by commas ("*": all), to check how the frontend shows failures. Changed while running
// by GET /mock/fail?mutations=updateCable,deleteCable (empty: none)
let failing = new Set((process.env.MOCK_FAIL ?? '').split(',').filter(Boolean));
const ORIGIN = `http://localhost:${PORT}`;
const ISSUER = `${ORIGIN}/realms/cable`;
const CLIENT_ID = 'cable-editor';
const DIST = path.resolve(process.env.MOCK_DIST ?? path.join(FRONTEND, 'dist'));
const SCHEMA_DIR = path.join(FRONTEND, 'graphql');

// ---------------------------------------------------------------- OIDC
const { publicKey, privateKey } = await generateKeyPair('RS256');
const jwk = { ...(await exportJWK(publicKey)), kid: 'k1', alg: 'RS256', use: 'sig' };
const nonces = new Map();

async function idToken(nonce) {
  return new SignJWT({ nonce, preferred_username: 'monteur', name: 'Mo Monteur' })
    .setProtectedHeader({ alg: 'RS256', kid: 'k1' })
    .setIssuer(ISSUER).setAudience(CLIENT_ID).setSubject('user-1')
    .setIssuedAt().setExpirationTime('2h').sign(privateKey);
}

// A refused request as the backend sends it: the reason in extensions.userError, worded by the
// frontend (see docs/fehlermeldungen.md)
const refuse = (code, data = {}) => {
  throw new GraphQLError(code, { extensions: { userError: { code, ...data } } });
};

// ---------------------------------------------------------------- data
// Plan 0 is the current state; plan 1 is open and changes two ports of Spleisskassette 2.
const schachtRows = [
  // id, name, [lat, lng] or null, type id (the real data is LV95, the backend delivers WGS84)
  [1, 'SCH 101 Bahnhofstrasse', [47.41963, 8.88611], 1], [2, 'SCH 102 Dorfplatz', [47.41988, 8.88618], 1],
  [3, 'SCH 103 Schulhaus', [47.42074, 8.88590], 2], [4, 'SCH 104 Industrie Nord', [47.41778, 8.88441], 0],
  [5, 'SCH 105 Werkhof', [47.41803, 8.88398], null],
];
const svg = (shape) => `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">${shape}</svg>`;
const schachtTypes = [
  { id: 0, name: 'Normschacht', icon: svg('<circle cx="50" cy="50" r="42" stroke="#01579b" stroke-width="8" fill="#e1f5fe"/>'), lkmapObjektart: 'SCHACHT_RUND', dimension1Mm: 1000, dimension2Mm: null },
  { id: 1, name: 'Kabelschacht', icon: svg('<rect x="10" y="20" width="80" height="60" rx="6" stroke="#2e7d32" stroke-width="8" fill="#e8f5e9"/>'), lkmapObjektart: 'SCHACHT_RECHTECKIG', dimension1Mm: 1200, dimension2Mm: 800 },
  { id: 2, name: 'Verteilkasten', icon: svg('<rect x="25" y="10" width="50" height="80" stroke="#c62828" stroke-width="8" fill="#ffebee"/>'), lkmapObjektart: 'BAUWERK', dimension1Mm: null, dimension2Mm: null },
];
const schachtTyp = (t) => t && { ...t, schachtCount: schachtRows.filter((r) => r[3] === t.id).length };
const schachtTypFromInput = ({ name, icon, lkmapObjektart, dimension1Mm, dimension2Mm }, typId) => {
  if (!name.trim()) refuse('NameMissing');
  if (name.trim().length > 20) refuse('NameTooLong', { max: 20 });
  if (schachtTypes.some((t) => t.id !== typId && t.name === name.trim())) refuse('NameTaken', { kind: 'SchachtTyp', name: name.trim() });
  if (dimension2Mm != null && dimension1Mm == null) refuse('Dimension2WithoutDimension1');
  if (dimension2Mm != null && dimension2Mm > dimension1Mm) refuse('DimensionsSwapped');
  if (icon && !/^\s*(<\?[^>]*\?>\s*)?<svg/.test(icon)) refuse('IconNotSvg');
  return { name: name.trim(), lkmapObjektart, dimension1Mm: dimension1Mm ?? null, dimension2Mm: dimension2Mm ?? null, ...(icon ? { icon } : {}) };
};

// swisstopo's approximate formulas (about 1 m); the backend lets PostGIS convert exactly
const toLv95 = ({ lat, lng }) => {
  const p = (lat * 3600 - 169028.66) / 10000, l = (lng * 3600 - 26782.5) / 10000;
  return {
    e: 2600072.37 + 211455.93 * l - 10938.51 * l * p - 0.36 * l * p * p - 44.54 * l ** 3,
    n: 1200147.07 + 308807.95 * p + 3745.25 * l * l + 76.63 * p * p - 194.56 * l * l * p + 119.79 * p ** 3,
  };
};
const toWgs84 = ({ e, n }) => {
  const y = (e - 2600000) / 1e6, x = (n - 1200000) / 1e6;
  const l = 2.6779094 + 4.728982 * y + 0.791484 * y * x + 0.1306 * y * x * x - 0.0436 * y ** 3;
  const p = 16.9023892 + 3.238272 * x - 0.270978 * y * y - 0.002528 * x * x - 0.0447 * y * y * x - 0.014 * x ** 3;
  return { lat: (p * 100) / 36, lng: (l * 100) / 36 };
};
// convertPoint's position: { lv95: { e, n } } or { wgs84: { lat, lng } }, as WGS84
const positionToWgs84 = (position) => {
  const wgs84 = position.lv95 ? toWgs84(position.lv95) : position.wgs84;
  const { e, n } = toLv95(wgs84);
  if (!(e >= 2480000 && e <= 2840000 && n >= 1070000 && n <= 1300000)) {
    refuse('PositionOutsideSwitzerland', { e, n });
  }
  return wgs84;
};
const schachtFromInput = ({ name, typeId, position }) => {
  if (!name.trim()) refuse('NameMissing');
  const wgs84 = position ? positionToWgs84(position) : null;
  return [name.trim(), wgs84 ? [wgs84.lat, wgs84.lng] : null, typeId ?? null];
};
// Owners of Schächte and ducts
const ownerRows = [
  { id: 1, name: 'Genossenschaft Glasfaser Berg', lkName: null, isDefault: true },
  { id: 2, name: 'Gemeinde Berg', lkName: null, isDefault: false },
  { id: 3, name: 'Hans Muster', lkName: 'Keine_Angabe', isDefault: false },
];
// Owner of a Schacht (default 1) and a duct (default 1), ducts delivered to the Leitungskataster
const schachtOwner = { 5: 2 };
const ductOwner = { 714: 2, 715: 3 };
// Lagebestimmung of a Schacht (default UNGENAU)
const schachtLage = { 1: 'GENAU' };
// Stores owner and Lagebestimmung of a Schacht's input
const storeSchachtDelivery = (id, { ownerId, lagebestimmung }) => {
  if (!ownerRows.some((o) => o.id === ownerId)) refuse('NotFound', { kind: 'Owner', id: ownerId });
  schachtOwner[id] = ownerId;
  schachtLage[id] = lagebestimmung;
};
const deliveredDucts = new Set([711, 713, 714]);
const ownerFromInput = ({ name, lkName }, ownerId) => {
  const clean = (value) => value?.trim() || null;
  if (!name.trim()) refuse('NameMissing');
  const others = ownerRows.filter((o) => o.id !== ownerId);
  if (others.some((o) => o.name === name.trim())) refuse('NameTaken', { kind: 'Owner', name: name.trim() });
  return { name: name.trim(), lkName: clean(lkName) };
};
const ownerOfSchacht = (id) => schachtOwner[id] ?? 1;
const ownerOfDuct = (id) => ductOwner[id] ?? 1;
const owner = (id) => {
  const row = ownerRows.find((o) => o.id === id);
  if (!row) return null;
  const ducts = ductRows.filter((r) => ownerOfDuct(r.id) === id);
  return {
    ...row,
    schachtCount: schachtRows.filter((r) => ownerOfSchacht(r[0]) === id).length,
    ductCount: ducts.length,
    deliveredDuctCount: ducts.filter((r) => deliveredDucts.has(r.id)).length,
  };
};
const cableRows = [
  // id, name, bundles, fibers, length, schacht a, schacht z
  [11, 'K-1001', 4, 12, 420.5, 1, 2], [12, 'K-1002', 2, 12, 310, 1, 3],
  [13, 'K-1003', 4, 12, 880, 1, 4], [14, 'K-1004', 1, 12, 150, 2, 5],
  [15, 'K-1005 Hausanschluss Gewerbe Nord', 1, 4, 95, 1, 5],
];
const panelRows = [
  // id, name, schacht, parent, order, [portType, count, labelPrefix]
  [21, 'ODF Rack 1', 1, null, 0, null],
  [22, 'Spleisskassette 1', 1, 21, 0, ['SPLICE', 12, 'S1-']],
  [23, 'Spleisskassette 2', 1, 21, 1, ['SPLICE', 12, 'S2-']],
  [24, 'Patchfeld LC', 1, 21, 2, ['CONNECTOR', 12, 'LC ']],
  [25, 'Loop-Kassette', 1, 21, 3, ['LOOP', 4, 'L']],
  [31, 'Muffe Dorfplatz', 2, null, 0, null],
  [32, 'Kassette A', 2, 31, 0, ['SPLICE', 12, 'A']],
];
const plans = [
  // Plan 0 is the baseline (current state); implemented plans are deleted
  { id: 0, name: 'Ist-Zustand', netboxActive: true },
  { id: 1, name: 'Erschliessung Gewerbe Nord', netboxActive: false },
  { id: 2, name: 'Umbau Dorfplatz 2025', netboxActive: false },
];
const devices = [{ id: 501, name: 'ODF-SCH101-1', deviceType: 'ODF 144', locationName: 'SCH 101', ports: 24 }];

const ports = []; // { id, panelId, orderNumber, label, portType }
let nextPort = 1000;
for (const [id, , , , , spec] of panelRows) {
  if (!spec) continue;
  for (let i = 1; i <= spec[1]; i++) {
    ports.push({ id: nextPort++, panelId: id, orderNumber: i, label: `${spec[2]}${i}`, portType: spec[0] });
  }
}
// Base usages (plan 0): [portId, side, cableId, bundle, fiber]
const baseUsage = [];
const planUsage = { 1: [] };
const portsOf = (panelId) => ports.filter((p) => p.panelId === panelId);
portsOf(22).forEach((p, i) => baseUsage.push([p.id, 'FRONT', 11, 1, i + 1]));
portsOf(23).slice(0, 6).forEach((p, i) => baseUsage.push([p.id, 'FRONT', 13, 2, i + 1]));
portsOf(24).slice(0, 6).forEach((p, i) => baseUsage.push([p.id, 'FRONT', 12, 1, i + 1]));
portsOf(32).slice(0, 8).forEach((p, i) => baseUsage.push([p.id, 'FRONT', 11, 1, i + 1]));
portsOf(32).slice(0, 4).forEach((p, i) => baseUsage.push([p.id, 'BACK', 14, 1, i + 1]));
portsOf(23).slice(6, 8).forEach((p, i) => planUsage[1].push([p.id, 'FRONT', 15, 1, i + 1]));
// Loops: fibers 1-4 of bundle 2 pass through from K-1002 to K-1001 uncut; plan 1 cuts 3 and 4
// (a row without cable removes the fiber) and splices them: K-1002 to K-1005 in Spleisskassette 2,
// K-1001 to a pigtail on the patch panel, whose front gets plugged
portsOf(25).forEach((p, i) => {
  baseUsage.push([p.id, 'FRONT', 12, 2, i + 1], [p.id, 'BACK', 11, 2, i + 1]);
  if (i >= 2) planUsage[1].push([p.id, 'FRONT', null, null, null], [p.id, 'BACK', null, null, null]);
});
portsOf(23).slice(6, 8).forEach((p, i) => planUsage[1].push([p.id, 'BACK', 12, 2, i + 3]));
planUsage[1].push([portsOf(24)[6].id, 'BACK', 11, 2, 3], [portsOf(24)[6].id, 'FRONT', 15, 1, 3]);
// Schacht 2: a splice moves to another fiber
planUsage[1].push([portsOf(32)[0].id, 'BACK', 14, 1, 5]);

function usageRow(planId, portId, side) {
  const inPlan = (planUsage[planId] || []).find((u) => u[0] === portId && u[1] === side);
  if (inPlan) return { row: inPlan, modified: true };
  const base = baseUsage.find((u) => u[0] === portId && u[1] === side);
  return base ? { row: base, modified: false } : null;
}

// ---------------------------------------------------------------- object graph
const schacht = (id) => {
  const r = schachtRows.find((s) => s[0] === id);
  if (!r) return null;
  return {
    id, name: r[1], typ: schachtTyp(schachtTypes.find((t) => t.id === r[3])) ?? null,
    position: r[2] && toLv95({ lat: r[2][0], lng: r[2][1] }),
    location: r[2] && { lat: r[2][0], lng: r[2][1] },
    owner: owner(ownerOfSchacht(id)),
    lagebestimmung: schachtLage[id] ?? 'UNGENAU',
    changedAt: '2026-09-27T08:15:00+00:00',
    connectingDuct: () => [],
    rootPanels: () => panelRows.filter((p) => p[2] === id && p[3] === null).map((p) => panel(p[0])),
    cable: ({ cableId }) => cablesAt(id).find((c) => c.cable.id === cableId) ?? null,
    cables: () => cablesAt(id),
  };
};
const cablesAt = (schachtId) =>
  cableRows.filter((c) => c[5] === schachtId || c[6] === schachtId).map((c) => cableEnd(c[0], schachtId));
// Each cable runs through a duct of its own (id 700 + cable id), bent a little between its
// Schächte; ducts created in the UI have no cables.
// ductRows: { id, description, a, z, points: [{ lat, lng }] between the Schächte }
const ductRows = cableRows.map((c) => {
  const [a, z] = [c[5], c[6]].map((id) => schachtRows.find((r) => r[0] === id)[2]);
  const bend = { lat: (a[0] + z[0]) / 2 + 0.00005 * (c[0] % 3 - 1), lng: (a[1] + z[1]) / 2 + 0.00005 };
  return { id: 700 + c[0], description: `Rohr ${c[1]}`, a: c[5], z: c[6], points: [bend] };
});
// A measured duct with its width
Object.assign(ductRows[0], { lagebestimmung: 'GENAU', widthMm: 110 });
const lv95Distance = (p, q) => { const a = toLv95(p), b = toLv95(q); return Math.hypot(a.e - b.e, a.n - b.n); };
const ductLine = (row) => {
  const a = schacht(row.a)?.location, z = schacht(row.z)?.location;
  if (!a || !z) return null;
  return [a, ...row.points, z];
};
const lineLength = (line) => line.slice(1).reduce((sum, p, i) => sum + lv95Distance(line[i], p), 0);
const duct = (row) => ({
  id: row.id, description: row.description, schachtA: schacht(row.a), schachtZ: schacht(row.z),
  length: ductLine(row) && lineLength(ductLine(row)),
  cables: () => cableRows.filter((c) => 700 + c[0] === row.id).map((c) => cable(c[0])),
  line: ductLine(row),
  owner: owner(ownerOfDuct(row.id)),
  leitungskataster: deliveredDucts.has(row.id),
  lagebestimmung: row.lagebestimmung ?? 'UNGENAU',
  widthMm: row.widthMm ?? null,
  changedAt: row.changedAt ?? '2026-09-27T08:15:00+00:00',
});
// The delivery to the Leitungskataster like lkmap/mod.rs: the delivered ducts of the whole
// network and the Schächte where one ends; the perimeter a box around them, the checksum over
// their data (and the owners' names, which go into the files)
const DATENHERR = 'CHE-123.456.789';
const lkmapExport = () => {
  const rows = ductRows.filter((r) => deliveredDucts.has(r.id));
  const ends = [...new Set(rows.flatMap((r) => [r.a, r.z]))].sort((a, b) => a - b);
  const ducts = rows.filter((r) => ductLine(r)).map(duct);
  const schaechte = ends.map(schacht).filter((s) => s.location);
  const points = [...ducts.flatMap((d) => d.line), ...schaechte.map((s) => s.location)];
  const files = points.length > 0;
  const [lats, lngs] = [points.map((p) => p.lat), points.map((p) => p.lng)];
  const [s0, n0, w0, e0] = [Math.min(...lats) - 0.0001, Math.max(...lats) + 0.0001, Math.min(...lngs) - 0.00015, Math.max(...lngs) + 0.00015];
  const uid = DATENHERR.toLowerCase().replaceAll('.', '-');
  const names = (id) => ownerRows.find((o) => o.id === id);
  const checksum = files ? createHash('sha256').update(JSON.stringify([
    ducts.map((d) => [d.id, d.line, d.changedAt, names(ownerOfDuct(d.id))]),
    schaechte.map((s) => [s.id, s.location, names(ownerOfSchacht(s.id))]),
  ])).digest('hex') : null;
  return {
    datenherr: DATENHERR,
    lkmap: files ? { fileName: `${uid}-kommunikation-lkmap.xtf`, xtf: '<TRANSFER/>' } : null,
    perimeter: files ? { fileName: `${uid}-zustaendigkeit-peri.xtf`, xtf: '<TRANSFER/>' } : null,
    perimeterArea: files ? [[s0, w0], [s0, e0], [n0, e0], [n0, w0], [s0, w0]].map(([lat, lng]) => ({ lat, lng })) : null,
    checksum,
    schaechte, ducts,
    schaechteWithoutPosition: ends.map(schacht).filter((s) => !s.location),
    ductsWithoutLine: rows.filter((r) => !ductLine(r)).map(duct),
    deliveries: [...lkmapDeliveries].sort((a, b) => b.id - a.id),
    // The mock's objects all changed on 25.09. (after the delivery in July)
    firstChangeSinceDelivery: lkmapDeliveries.some((d) => d.deliveredAt && d.checksum !== checksum) ? '2026-09-25T09:30:00+00:00' : null,
  };
};
// Delivered in July, the data changed since; a download not confirmed yet
const lkmapDeliveries = [
  { id: 1, createdAt: '2026-07-14T13:05:00+00:00', createdBy: 'monteur', schachtCount: 3, ductCount: 1, checksum: '5f1c0a7e'.repeat(8), deliveredAt: '2026-07-14T13:20:00+00:00' },
];
const ductOfCable = (c) => duct(ductRows.find((r) => r.id === 700 + c[0]));
// checkDuctLine's fitting, like graphql/duct_line.rs (distances in LV95)
const fitLine = (aId, zId, { system, points }) => {
  const a = schacht(aId)?.location, z = schacht(zId)?.location;
  if (!a || !z) refuse('SchachtWithoutPosition', { schacht: schacht(a ? zId : aId)?.name ?? String(a ? zId : aId) });
  if (lv95Distance(a, z) < 0.5) refuse('SchaechteAtSamePlace');
  let line = points.map(({ x, y }) => (system === 'WGS84' ? { lat: y, lng: x }
    : toWgs84(system === 'LV95' ? { e: x, n: y } : { e: x + 2000000, n: y + 1000000 })));
  const reversed = lv95Distance(line[0], z) + lv95Distance(line.at(-1), a) < lv95Distance(line[0], a) + lv95Distance(line.at(-1), z);
  if (reversed) line = line.reverse();
  const startDistance = lv95Distance(line[0], a), endDistance = lv95Distance(line.at(-1), z);
  let trimmed = [...line];
  if (trimmed.length > 1 && endDistance < 0.5) trimmed.pop();
  if (startDistance < 0.5) trimmed.shift();
  if (trimmed.length === 1) trimmed = line;
  const full = [a, ...trimmed, z];
  return {
    points: trimmed, line: full, reversed, removedEnds: line.length - trimmed.length, startDistance, endDistance,
    length: lineLength(full), needsConfirmation: Math.max(startDistance, endDistance) > 10,
  };
};
const checkedPoints = (aId, zId, line, confirmed) => {
  if (!line) return [];
  const fitted = fitLine(aId, zId, line);
  if (fitted.needsConfirmation && !confirmed) {
    refuse('LineNeedsConfirmation', { startDistance: fitted.startDistance, endDistance: fitted.endDistance });
  }
  return fitted.points;
};
const ductFromInput = ({ schachtA, schachtZ, description, ownerId, leitungskataster, lagebestimmung, widthMm }) => {
  if (schachtA === schachtZ) refuse('SameSchachtAtBothEnds');
  if (widthMm != null && (widthMm < 0 || widthMm > 4000)) refuse('WidthOutOfRange', { max: 4000 });
  if (!ownerRows.some((o) => o.id === ownerId)) refuse('NotFound', { kind: 'Owner', id: ownerId });
  return { a: schachtA, z: schachtZ, description: description?.trim() || null, ownerId, leitungskataster, lagebestimmung, widthMm: widthMm ?? null };
};
// Stores what ductFromInput checked in the owner and delivery tables of the mock
const storeDelivery = (row) => {
  ductOwner[row.id] = row.ownerId;
  if (row.leitungskataster) deliveredDucts.add(row.id); else deliveredDucts.delete(row.id);
  row.changedAt = new Date().toISOString();
};
const cable = (id) => {
  const c = cableRows.find((r) => r[0] === id);
  return {
    id, name: c[1], bundleCount: c[2], fiberCount: c[3], length: c[4],
    path: () => cablePath(id, c[5]),
    line: ductOfCable(c).line,
    end: ({ schachtId }) => cableEnd(id, schachtId),
  };
};
const farOf = (cableId, schachtId) => {
  const c = cableRows.find((r) => r[0] === cableId);
  return c[5] === schachtId ? c[6] : c[5];
};
const cablePath = (cableId, fromSchacht) => {
  const c = cableRows.find((r) => r[0] === cableId);
  const far = farOf(cableId, fromSchacht);
  return {
    nearSchacht: () => schacht(fromSchacht), nearEnd: () => cableEnd(cableId, fromSchacht),
    segments: () => [{ duct: ductOfCable(c), farSchacht: schacht(far), sequence: 0 }],
    farSchacht: () => schacht(far), farEnd: () => cableEnd(cableId, far),
  };
};
const cableEnd = (cableId, schachtId) => ({
  cable: () => cable(cableId), schacht: () => schacht(schachtId),
  path: () => cablePath(cableId, schachtId),
  usedPorts: ({ planId }) => fiberUsagesAt(planId, cableId, schachtId),
  fibers: () => {
    const c = cableRows.find((r) => r[0] === cableId);
    const out = [];
    for (let b = 1; b <= c[2]; b++) for (let f = 1; f <= c[3]; f++) out.push(fiberEnd(cableId, schachtId, b, f));
    return out;
  },
});
const fiberUsagesAt = (planId, cableId, schachtId) => {
  const out = [];
  for (const p of ports) {
    if (panel(p.panelId).schachtId !== schachtId) continue;
    for (const side of ['FRONT', 'BACK']) {
      const u = usageRow(planId, p.id, side);
      if (u && u.row[2] === cableId) out.push(portUsage(planId, p.id, side));
    }
  }
  return out;
};
const fiberEnd = (cableId, schachtId, bundle, fiber) => ({
  cable: () => cableEnd(cableId, schachtId), bundle, fiber,
  usedPort: ({ planId }) =>
    fiberUsagesAt(planId, cableId, schachtId).find((u) => u.fiber.bundle === bundle && u.fiber.fiber === fiber) ?? null,
  otherEnd: () => fiberEnd(cableId, farOf(cableId, schachtId), bundle, fiber),
});
const plan = (id) => {
  const p = plans.find((x) => x.id === id);
  if (!p) return null;
  return {
    ...p,
    isBaseline: id === 0,
    rootPanels: () => panelRows.filter((r) => r[3] === null).map((r) => plannedPanel(r[0], id)),
    panel: ({ panelId }) => plannedPanel(panelId, id),
    usage: () => (planUsage[id] || []).map((u) => portUsage(id, u[0], u[1])),
    // Like the backend: the ports the plan's rows change, by panel and position
    changedPorts: () => {
      const fiberOf = (row) => (row && row[2] !== null ? { cable: () => cable(row[2]), bundle: row[3], fiber: row[4] } : null);
      const same = (a, b) => (a?.[2] ?? null) === (b?.[2] ?? null) && a?.[3] === b?.[3] && a?.[4] === b?.[4];
      const rows = planUsage[id] || [];
      return [...new Set(rows.map((u) => u[0]))]
        .map((portId) => {
          const side = (s) => {
            const current = baseUsage.find((u) => u[0] === portId && u[1] === s) ?? null;
            const planned = rows.find((u) => u[0] === portId && u[1] === s) ?? current;
            return [current, planned];
          };
          const [currentFront, plannedFront] = side('FRONT');
          const [currentBack, plannedBack] = side('BACK');
          if (same(currentFront, plannedFront) && same(currentBack, plannedBack)) return null;
          return {
            port: panelPort(portId), currentFront: fiberOf(currentFront), currentBack: fiberOf(currentBack),
            plannedFront: fiberOf(plannedFront), plannedBack: fiberOf(plannedBack),
          };
        })
        .filter(Boolean)
        .sort((a, b) => a.port.panelId - b.port.panelId || a.port.orderNumber - b.port.orderNumber);
    },
  };
};
// The last run of the automatic Netbox sync, as MOCK_NETBOX says
const netboxSync = () => {
  const connectors = ports.filter((p) => p.portType === 'CONNECTOR');
  const rearPort = (name) => ({ id: 9000, name, deviceName: 'ODF-SCH101-1', locationName: 'SCH 101' });
  const issues = NETBOX !== 'ISSUES' ? [] : [
    { __typename: 'MissingNetboxReferenceError', port: () => panelPort(connectors[0].id) },
    { __typename: 'RoutingLoopError', port: () => panelPort(connectors[1].id) },
    {
      __typename: 'AsymmetricDuplexError', startNetboxPort: rearPort('RP 1'),
      connections: [1, 2].map((i) => ({
        targetNetboxPort: rearPort(`RP ${i + 1}`),
        pairs: [{ sourcePort: () => panelPort(connectors[2].id), targetPort: () => panelPort(connectors[2 + i].id) }],
      })),
    },
  ];
  const error = NETBOX !== 'FEHLER' ? null : {
    message: 'error sending request for url (http://netbox.local/graphql/)',
    extensions: JSON.stringify({ origin: { library: 'cynic (Netbox)', location: 'cable-editor-backend/src/netbox/fetch.rs:117', id: '1a0e8c917ee0a' } }),
  };
  const state = { OK: 'SYNCHRON', ISSUES: 'NICHT_SYNCHRON', FEHLER: 'FEHLER' }[NETBOX] ?? 'AUSSTEHEND';
  return {
    state, pending: NETBOX === 'FEHLER',
    lastRun: state === 'AUSSTEHEND' ? null : '2026-09-28T06:00:00+00:00',
    retryAt: NETBOX === 'FEHLER' ? '2026-09-28T06:02:00+00:00' : null,
    activePlan: () => plan(plans.find((p) => p.netboxActive)?.id ?? 0), issues, error,
  };
};
const portUsage = (planId, portId, side) => {
  const u = usageRow(planId, portId, side);
  if (!u) return null;
  const [, , cableId, bundle, fiber] = u.row;
  return {
    side, modifiedInPlan: u.modified,
    // A row of a plan without cable removes the fiber
    fiber: cableId === null ? null : { bundle, fiber, cable: () => cable(cableId) },
    port: () => panelPort(portId), plan: () => plan(planId),
    otherSide: ({ planId: plan }) => effectiveUsage(plan, portId, side === 'FRONT' ? 'BACK' : 'FRONT'),
    // Simplified trace: the fiber ends at this port
    cableSideEndPort: () => null, panelSideEndPort: () => portUsage(planId, portId, side),
  };
};
// Like the backend's effective_port_usage: a removed fiber is no usage
const effectiveUsage = (planId, portId, side) => {
  const u = portUsage(planId, portId, side);
  return u?.fiber ? u : null;
};
const panelPort = (id) => {
  const p = ports.find((x) => x.id === id);
  return { ...p, panel: () => panel(p.panelId), netboxPort: () => null };
};
const panel = (id) => {
  const r = panelRows.find((p) => p[0] === id);
  if (!r) return null;
  const [, name, schachtId, parentId, order] = r;
  const children = () => panelRows.filter((p) => p[3] === id).sort((a, b) => a[4] - b[4]).map((p) => panel(p[0]));
  const self = {
    id, name, schachtId, parentId, parentOrder: order,
    schacht: () => schacht(schachtId),
    parent: () => (parentId === null ? null : panel(parentId)),
    parentChain: () => {
      const chain = [];
      for (let p = parentId; p !== null; p = panelRows.find((x) => x[0] === p)[3]) chain.unshift(panel(p));
      return chain;
    },
    children,
    // Like the backend: the panels at the same level, without this one
    siblings: () =>
      (parentId === null
        ? panelRows.filter((p) => p[2] === schachtId && p[3] === null).map((p) => panel(p[0]))
        : panel(parentId).children()
      ).filter((p) => p.id !== id),
    allChildrenRecursive: () => children().flatMap((c) => [c, ...c.allChildrenRecursive()]),
    ports: ({ portType } = {}) => portsOf(id).filter((p) => !portType || p.portType === portType).map((p) => panelPort(p.id)),
    countPorts: ({ portType } = {}) => portsOf(id).filter((p) => !portType || p.portType === portType).length,
    netboxDevice: () => (id === 21 ? device(501) : null),
  };
  return self;
};
const plannedPanel = (panelId, planId) => {
  const p = panel(panelId);
  if (!p) return null;
  return {
    panel: p, plan: () => plan(planId),
    parent: () => (p.parentId === null ? null : plannedPanel(p.parentId, planId)),
    children: () => p.children().map((c) => plannedPanel(c.id, planId)),
    ports: () => portsOf(panelId).map((x) => ({
      ...x,
      usage: ({ side }) => effectiveUsage(planId, x.id, side),
      currentUsage: ({ side }) => effectiveUsage(0, x.id, side),
    })),
    allChildrenRecursive: () => p.allChildrenRecursive().map((c) => plannedPanel(c.id, planId)),
  };
};
const device = (id) => {
  const d = devices.find((x) => x.id === id);
  const self = {
    ...d,
    rearPorts: () => Array.from({ length: d.ports }, (_, i) => ({ id: 600 + i, name: `RP ${i + 1}`, fiberPorts: [], device: self })),
  };
  return self;
};

const root = {
  currentUser: {
    displayName: 'Mo Monteur',
    groups: { ADMIN: ['cable-admins'], PLANNER: ['cable-planners'] }[ROLE] ?? [],
    preferredUsername: 'monteur',
    picture: '',
    role: ROLE,
  },
  listSchacht: () => schachtRows.map((s) => schacht(s[0])),
  schacht: ({ schachtId }) => schacht(schachtId),
  listSchachtTyp: () => schachtTypes.map(schachtTyp),
  schachtTyp: ({ typId }) => schachtTyp(schachtTypes.find((t) => t.id === typId)) ?? null,
  convertPoint: ({ position }) => {
    const wgs84 = positionToWgs84(position);
    return { lv95: toLv95(wgs84), wgs84 };
  },
  listCable: () => cableRows.map((c) => cable(c[0])),
  cable: ({ cableId }) => (cableRows.some((c) => c[0] === cableId) ? cable(cableId) : null),
  listDuct: () => ductRows.map(duct),
  duct: ({ ductId }) => { const row = ductRows.find((r) => r.id === ductId); return row ? duct(row) : null; },
  checkDuctLine: ({ schachtA, schachtZ, line }) => fitLine(schachtA, schachtZ, line),
  lkmapExport: () => lkmapExport(),
  downloadLkmap: () => {
    const exported = lkmapExport();
    if (!exported.checksum) refuse('NothingToDeliver');
    let delivery = lkmapDeliveries.find((d) => !d.deliveredAt && d.checksum === exported.checksum);
    if (!delivery) {
      delivery = { id: Math.max(0, ...lkmapDeliveries.map((d) => d.id)) + 1, createdAt: new Date().toISOString(), createdBy: 'monteur',
        schachtCount: exported.schaechte.length, ductCount: exported.ducts.length, checksum: exported.checksum, deliveredAt: null };
      lkmapDeliveries.push(delivery);
    }
    // Not a real ZIP, the mock has no files to pack
    const zip = (file) => ({ fileName: file.fileName.replace(/\.xtf$/, '.zip'), content: Buffer.from(file.xtf).toString('base64') });
    return { delivery, files: [zip(exported.lkmap), zip(exported.perimeter)] };
  },
  setLkmapDelivered: ({ deliveryId, delivered }) => {
    const delivery = lkmapDeliveries.find((d) => d.id === deliveryId);
    if (!delivery) refuse('NotFound', { kind: 'LkmapDelivery', id: deliveryId });
    delivery.deliveredAt = delivered ? (delivery.deliveredAt ?? new Date().toISOString()) : null;
    return delivery;
  },
  listOwner: () => ownerRows.map((o) => owner(o.id)).sort((a, b) => a.name.localeCompare(b.name)),
  listPlan: () => plans.map((p) => plan(p.id)),
  netboxSync: () => netboxSync(),
  plan: ({ planId }) => plan(planId),
  panel: ({ panelId }) => panel(panelId),
  netboxDevices: () => devices.map((d) => device(d.id)),
  netboxDevice: ({ netboxDeviceId }) => device(netboxDeviceId),
  // mutations
  createCable: () => cable(11), updateCable: () => cable(11), deleteCable: () => true,
  createPanel: () => true, updatePanels: () => true, createPlan: () => true,
  updateCabinetPanels: () => true, updatePanelPorts: () => true, setPortUsage: () => true,
  updatePlan: ({ planId }) => plan(planId), implementPlan: ({ planId }) => plan(planId),
  setNetboxActivePlan: ({ planId }) => {
    if (!plan(planId)) refuse('NotFound', { kind: 'Plan', id: planId });
    for (const p of plans) p.netboxActive = p.id === planId;
    return plan(planId);
  },
  syncNetbox: () => true,
  createSchacht: ({ schacht: input }) => {
    const id = Math.max(...schachtRows.map((r) => r[0])) + 1;
    const values = schachtFromInput(input);
    storeSchachtDelivery(id, input);
    schachtRows.push([id, ...values]);
    return schacht(id);
  },
  updateSchacht: ({ schachtId, schacht: input }) => {
    const row = schachtRows.find((r) => r[0] === schachtId);
    if (!row) refuse('NotFound', { kind: 'Schacht', id: schachtId });
    const values = schachtFromInput(input);
    storeSchachtDelivery(schachtId, input);
    row.splice(1, 3, ...values);
    return schacht(schachtId);
  },
  createDuct: ({ duct: input, line, confirmed }) => {
    const row = { id: Math.max(...ductRows.map((r) => r.id)) + 1, ...ductFromInput(input) };
    row.points = checkedPoints(row.a, row.z, line, confirmed);
    ductRows.push(row);
    storeDelivery(row);
    return duct(row);
  },
  updateDuct: ({ ductId, duct: input }) => {
    const row = ductRows.find((r) => r.id === ductId);
    const changes = ductFromInput(input);
    if ((changes.a !== row.a || changes.z !== row.z) && duct(row).cables().length) {
      refuse('DuctEndsFixed');
    }
    Object.assign(row, changes);
    storeDelivery(row);
    return duct(row);
  },
  setDuctLine: ({ ductId, line, confirmed }) => {
    const row = ductRows.find((r) => r.id === ductId);
    row.points = checkedPoints(row.a, row.z, line, confirmed);
    return duct(row);
  },
  deleteDuct: ({ ductId }) => {
    const row = ductRows.find((r) => r.id === ductId);
    const cables = duct(row).cables().length;
    if (cables) refuse('DuctHasCables', { cables });
    ductRows.splice(ductRows.indexOf(row), 1);
    return true;
  },
  createSchachtTyp: ({ typ: input }) => {
    const row = { id: Math.max(...schachtTypes.map((t) => t.id)) + 1, icon: svg('<circle cx="50" cy="50" r="42" stroke="#4d5258" stroke-width="8" fill="#f0f0f0"/>'), ...schachtTypFromInput(input) };
    schachtTypes.push(row);
    return schachtTyp(row);
  },
  updateSchachtTyp: ({ typId, typ: input }) => {
    const row = schachtTypes.find((t) => t.id === typId);
    Object.assign(row, schachtTypFromInput(input, typId));
    return schachtTyp(row);
  },
  deleteSchachtTyp: ({ typId }) => {
    const { schachtCount } = schachtTyp(schachtTypes.find((t) => t.id === typId));
    if (schachtCount) refuse('SchachtTypReferenced', { schaechte: schachtCount });
    schachtTypes.splice(schachtTypes.findIndex((t) => t.id === typId), 1);
    return true;
  },
  createOwner: ({ owner: input }) => {
    const row = { id: Math.max(...ownerRows.map((o) => o.id)) + 1, ...ownerFromInput(input), isDefault: false };
    ownerRows.push(row);
    return owner(row.id);
  },
  updateOwner: ({ ownerId, owner: input }) => {
    Object.assign(ownerRows.find((o) => o.id === ownerId), ownerFromInput(input, ownerId));
    return owner(ownerId);
  },
  setDefaultOwner: ({ ownerId }) => {
    ownerRows.forEach((o) => { o.isDefault = o.id === ownerId; });
    return owner(ownerId);
  },
  deleteOwner: ({ ownerId }) => {
    const { isDefault, schachtCount, ductCount } = owner(ownerId);
    if (isDefault) refuse('DefaultOwnerNotDeletable');
    if (schachtCount || ductCount) refuse('OwnerReferenced', { schaechte: schachtCount, ducts: ductCount });
    ownerRows.splice(ownerRows.findIndex((o) => o.id === ownerId), 1);
    return true;
  },
  deleteSchacht: ({ schachtId }) => {
    const panels = panelRows.filter((p) => p[2] === schachtId).length;
    const ducts = ductRows.filter((r) => r.a === schachtId || r.z === schachtId).length;
    if (panels || ducts) refuse('SchachtReferenced', { panels, ducts });
    schachtRows.splice(schachtRows.findIndex((r) => r[0] === schachtId), 1);
    return true;
  },
};

// The mutations of MOCK_FAIL fail with an unexpected error, as a diesel error would
const isMutation = (name) => /^(create|update|delete|set|implement|sync|download)/.test(name);
for (const [name, resolver] of Object.entries(root)) {
  if (typeof resolver !== 'function' || !isMutation(name)) continue;
  root[name] = (...args) => {
    if (failing.has('*') || failing.has(name)) {
      throw new GraphQLError('db error', {
        extensions: { origin: { library: 'diesel', location: `mock/${name}.rs:1`, id: 'mock-0001' } },
      });
    }
    return resolver(...args);
  };
}

// graphql-js 16 predefines @oneOf
const load = (f) => fs.readFileSync(path.join(SCHEMA_DIR, f), 'utf8').replace(/"""[^"]*"""\s*directive @oneOf[^\n]*\n/, '').replace(/ @oneOf/g, '');
const authSchema = buildSchema(load('authenticated_schema.graphql'));
const anonSchema = buildSchema(load('anonymous_schema.graphql'));
const anonRoot = { authentication: { clientId: CLIENT_ID, issuerUrl: ISSUER, scopes: ['openid', 'profile'] } };

// ---------------------------------------------------------------- http
const body = (req) => new Promise((ok) => { let b = ''; req.on('data', (c) => (b += c)); req.on('end', () => ok(b)); });
const json = (res, obj, status = 200) => { res.writeHead(status, { 'content-type': 'application/json' }); res.end(JSON.stringify(obj)); };
const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.css': 'text/css', '.woff2': 'font/woff2', '.svg': 'image/svg+xml', '.png': 'image/png', '.json': 'application/json', '.webmanifest': 'application/manifest+json', '.mjs': 'text/javascript' };

let netboxQueries = 0;
http.createServer(async (req, res) => {
  const url = new URL(req.url, ORIGIN);
  const p = url.pathname;
  // The login may come from another origin, the real backend's app (local/realdb)
  if (p.startsWith('/realms/')) {
    res.setHeader('access-control-allow-origin', '*');
    res.setHeader('access-control-allow-headers', '*');
    if (req.method === 'OPTIONS') return res.writeHead(204).end();
  }
  try {
    if (p === '/graphql' || p === '/graphql_anonymous') {
      const { query, variables, operationName } = JSON.parse(await body(req));
      const anon = p === '/graphql_anonymous';
      const result = await graphql({ schema: anon ? anonSchema : authSchema, source: query, rootValue: anon ? anonRoot : root, variableValues: variables, operationName });
      if (result.errors) console.log('GQL ERR', operationName, JSON.stringify(result.errors).slice(0, 500));
      return json(res, result);
    }
    if (p === '/mock/fail') {
      failing = new Set((url.searchParams.get('mutations') ?? '').split(',').filter(Boolean));
      return json(res, { failing: [...failing] });
    }
    if (p === '/realms/cable/.well-known/openid-configuration') {
      return json(res, {
        issuer: ISSUER, authorization_endpoint: `${ISSUER}/auth`, token_endpoint: `${ISSUER}/token`,
        userinfo_endpoint: `${ISSUER}/userinfo`, jwks_uri: `${ISSUER}/jwks`, end_session_endpoint: `${ISSUER}/logout`,
        response_types_supported: ['code'], subject_types_supported: ['public'],
        id_token_signing_alg_values_supported: ['RS256'],
      });
    }
    if (p === '/realms/cable/jwks') return json(res, { keys: [jwk] });
    if (p === '/realms/cable/auth') {
      if (ROLE === 'DENIED') {
        // Like a provider refusing a user whose groups aren't allowed for the client
        const back = new URL(url.searchParams.get('redirect_uri'));
        back.searchParams.set('error', 'access_denied');
        back.searchParams.set('state', url.searchParams.get('state'));
        res.writeHead(302, { location: back.toString() });
        return res.end();
      }
      const code = `code-${Math.random().toString(36).slice(2)}`;
      nonces.set(code, url.searchParams.get('nonce'));
      const back = new URL(url.searchParams.get('redirect_uri'));
      back.searchParams.set('code', code);
      back.searchParams.set('state', url.searchParams.get('state'));
      res.writeHead(302, { location: back.toString() });
      return res.end();
    }
    if (p === '/realms/cable/token') {
      const form = new URLSearchParams(await body(req));
      const nonce = nonces.get(form.get('code'));
      // A JWT for the client as access token too, so the real backend accepts it (local/realdb)
      return json(res, { access_token: await idToken(), token_type: 'Bearer', expires_in: 7200, id_token: await idToken(nonce) });
    }
    if (p === '/realms/cable/userinfo') return json(res, { sub: 'user-1', preferred_username: 'monteur' });
    // A Netbox without circuits for the real backend's automatic sync (local/realdb): enough for a
    // plan without connectors, whose sync only lists the circuits to delete the stale ones
    if (p === '/netbox/graphql/') {
      netboxQueries++;
      return json(res, { data: { circuit_list: [] } });
    }
    if (p === '/netbox/queries') return json(res, { count: netboxQueries });
    // static, SPA fallback
    let file = path.join(DIST, decodeURIComponent(p));
    if (!file.startsWith(DIST) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) file = path.join(DIST, 'index.html');
    res.writeHead(200, { 'content-type': types[path.extname(file)] || 'application/octet-stream' });
    fs.createReadStream(file).pipe(res);
  } catch (e) {
    console.log('ERR', p, e);
    json(res, { error: String(e) }, 500);
  }
}).listen(PORT, () => console.log(`mock on ${ORIGIN}`));
