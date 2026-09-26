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
import { buildSchema, graphql } from 'graphql';
import { generateKeyPair, exportJWK, SignJWT } from 'jose';

const FRONTEND = path.join(path.dirname(fileURLToPath(import.meta.url)), '../../cable-editor-frontend');
const PORT = Number(process.env.MOCK_PORT ?? 8099);
// Role of the mock user (READER, PLANNER or ADMIN), to check what the frontend hides; DENIED
// refuses the login like a provider that doesn't allow the user's groups
const ROLE = process.env.MOCK_ROLE ?? 'ADMIN';
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

// ---------------------------------------------------------------- data
// Plan 0 is the current state; plan 1 is open and changes two ports of Spleisskassette 2.
const schachtRows = [
  // id, name, [lat, lng] (the real data is LV95, the backend delivers WGS84)
  [1, 'SCH 101 Bahnhofstrasse', [47.41963, 8.88611]], [2, 'SCH 102 Dorfplatz', [47.41988, 8.88618]],
  [3, 'SCH 103 Schulhaus', [47.42074, 8.88590]], [4, 'SCH 104 Industrie Nord', [47.41778, 8.88441]],
  [5, 'SCH 105 Werkhof', [47.41803, 8.88398]],
];
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
  { id: 0, name: 'Ist-Zustand' },
  { id: 1, name: 'Erschliessung Gewerbe Nord' },
  { id: 2, name: 'Umbau Dorfplatz 2025' },
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
    id, name: r[1], typ: null, position: { x: 2700000 + id * 10, y: 1260000 + id * 10 },
    location: { lat: r[2][0], lng: r[2][1] },
    connectingDuct: () => [],
    rootPanels: () => panelRows.filter((p) => p[2] === id && p[3] === null).map((p) => panel(p[0])),
    cable: ({ cableId }) => cablesAt(id).find((c) => c.cable.id === cableId) ?? null,
    cables: () => cablesAt(id),
  };
};
const cablesAt = (schachtId) =>
  cableRows.filter((c) => c[5] === schachtId || c[6] === schachtId).map((c) => cableEnd(c[0], schachtId));
// Each cable runs through a duct of its own, bent a little between its Schächte
const ductLine = (c) => {
  const a = schacht(c[5]).location, z = schacht(c[6]).location;
  const bend = { lat: (a.lat + z.lat) / 2 + 0.00005 * (c[0] % 3 - 1), lng: (a.lng + z.lng) / 2 + 0.00005 };
  return [a, bend, z];
};
const duct = (c) => ({
  id: 700 + c[0], description: `Rohr ${c[1]}`, schachtA: schacht(c[5]), schachtZ: schacht(c[6]), length: c[4],
  ownWork: c[0] !== 13, cables: () => [cable(c[0])], line: ductLine(c),
});
const cable = (id) => {
  const c = cableRows.find((r) => r[0] === id);
  return {
    id, name: c[1], bundleCount: c[2], fiberCount: c[3], length: c[4],
    path: () => cablePath(id, c[5]),
    line: ductLine(c),
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
    segments: () => [{ duct: duct(c), farSchacht: schacht(far), sequence: 0 }],
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
  };
};
const portUsage = (planId, portId, side) => {
  const u = usageRow(planId, portId, side);
  if (!u) return null;
  const [, , cableId, bundle, fiber] = u.row;
  return {
    side, modifiedInPlan: u.modified,
    fiber: { bundle, fiber, cable: () => cable(cableId) },
    port: () => panelPort(portId), plan: () => plan(planId),
    otherSide: () => portUsage(planId, portId, side === 'FRONT' ? 'BACK' : 'FRONT'),
    // Simplified trace: the fiber ends at this port
    cableSideEndPort: () => null, panelSideEndPort: () => portUsage(planId, portId, side),
  };
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
      usage: ({ side }) => portUsage(planId, x.id, side),
      currentUsage: ({ side }) => portUsage(0, x.id, side),
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
  listSchachtTyp: () => [],
  listCable: () => cableRows.map((c) => cable(c[0])),
  cable: ({ cableId }) => (cableRows.some((c) => c[0] === cableId) ? cable(cableId) : null),
  listDuct: () => cableRows.map(duct),
  listPlan: () => plans.map((p) => plan(p.id)),
  plan: ({ planId }) => plan(planId),
  panel: ({ panelId }) => panel(panelId),
  netboxDevices: () => devices.map((d) => device(d.id)),
  netboxDevice: ({ netboxDeviceId }) => device(netboxDeviceId),
  // mutations
  createCable: () => cable(11), updateCable: () => cable(11), deleteCable: () => true,
  createPanel: () => true, updatePanels: () => true, createPlan: () => true,
  updateCabinetPanels: () => true, updatePanelPorts: () => true, setPortUsage: () => true,
  updatePlan: ({ planId }) => plan(planId), implementPlan: ({ planId }) => plan(planId),
  syncPlanToNetbox: () => [],
};

// graphql-js 16 predefines @oneOf
const load = (f) => fs.readFileSync(path.join(SCHEMA_DIR, f), 'utf8').replace(/"""[^"]*"""\s*directive @oneOf[^\n]*\n/, '').replace(/ @oneOf/g, '');
const authSchema = buildSchema(load('authenticated_schema.graphql'));
const anonSchema = buildSchema(load('anonymous_schema.graphql'));
const anonRoot = { authentication: { clientId: CLIENT_ID, issuerUrl: ISSUER, scopes: ['openid', 'profile'] } };

// ---------------------------------------------------------------- http
const body = (req) => new Promise((ok) => { let b = ''; req.on('data', (c) => (b += c)); req.on('end', () => ok(b)); });
const json = (res, obj, status = 200) => { res.writeHead(status, { 'content-type': 'application/json' }); res.end(JSON.stringify(obj)); };
const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.css': 'text/css', '.woff2': 'font/woff2', '.svg': 'image/svg+xml', '.png': 'image/png', '.json': 'application/json', '.mjs': 'text/javascript' };

http.createServer(async (req, res) => {
  const url = new URL(req.url, ORIGIN);
  const p = url.pathname;
  try {
    if (p === '/graphql' || p === '/graphql_anonymous') {
      const { query, variables, operationName } = JSON.parse(await body(req));
      const anon = p === '/graphql_anonymous';
      const result = await graphql({ schema: anon ? anonSchema : authSchema, source: query, rootValue: anon ? anonRoot : root, variableValues: variables, operationName });
      if (result.errors) console.log('GQL ERR', operationName, JSON.stringify(result.errors).slice(0, 500));
      return json(res, result);
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
      return json(res, { access_token: 'mock-access', token_type: 'Bearer', expires_in: 7200, id_token: await idToken(nonce) });
    }
    if (p === '/realms/cable/userinfo') return json(res, { sub: 'user-1', preferred_username: 'monteur' });
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
