// Runtime check of the frontend served by the mock backend (local/mock/server.mjs) against
// docs/frontend-konventionen.md: opens every route as a phone and on the desktop and checks what
// can be told from the outside. Started by run.sh (which starts the mock with MOCK_ROLE).
//
//   node local/konventionen/pages.mjs
//
// Checks with `step: N` are not implemented yet: their hits are only counted ("pending"), the
// commit of step N of the doc's "Umsetzung" removes the `step`. Exit code 1 if a switched-on check
// has hits.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium, devices } from '../mock/node_modules/playwright/index.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ORIGIN = `http://localhost:${process.env.MOCK_PORT ?? 8099}`;
const ROLE = process.env.MOCK_ROLE ?? 'ADMIN';
const TOAST_TITLE = /konnte(n)? nicht (gespeichert|angelegt|gelöscht|geladen|angestossen|geändert|abgeschlossen|aktiviert) werden/;
const NEW_ID = '00000000-0000-4000-8000-000000000001';

// Every route of the app. `v` names the variants of the enums in pages/router.rs the path shows,
// the coverage test fails if one is missing here. `admin`/`planner`: the role the page needs (any
// other role gets "Keine Berechtigung", which is checked as well).
const ROUTES = [
  { v: ['AppRoute::ListOfPlans'], path: '/listofplans' },
  { v: ['AppRoute::Plan', 'PlanView::Edit'], path: '/plan/1/edit' },
  { v: ['PlanView::ListOfCabinets'], path: '/plan/0/listofcabinets' },
  { v: ['PlanView::NewCabinet'], path: `/plan/0/newcabinet/${NEW_ID}`, needs: 'PLANNER' },
  { v: ['PlanView::Cabinet', 'CabinetView::Overview'], path: '/plan/0/cabinet/1/overview' },
  { v: ['CabinetView::Properties'], path: '/plan/0/cabinet/1/properties' },
  { v: ['CabinetView::Edit'], path: '/plan/0/cabinet/1/edit', needs: 'PLANNER' },
  { v: ['PlanView::ListOfCables'], path: '/plan/0/listofcables' },
  { v: ['PlanView::NewCable'], path: `/plan/0/newcable/${NEW_ID}`, needs: 'PLANNER' },
  { v: ['PlanView::Cable', 'CableView::Edit'], path: '/plan/0/cable/11/edit' },
  { v: ['PlanView::Map'], path: '/plan/0/map' },
  { v: ['PlanView::ListOfDucts'], path: '/plan/0/listofducts' },
  { v: ['PlanView::NewDuct'], path: `/plan/0/newduct/${NEW_ID}`, needs: 'PLANNER' },
  { v: ['PlanView::Duct', 'DuctView::Show'], path: '/plan/0/duct/711/show' },
  { v: ['DuctView::Properties'], path: '/plan/0/duct/711/properties' },
  { v: ['PlanView::ListOfOwners'], path: '/plan/0/listofowners' },
  { v: ['PlanView::ListOfCabinetTypes'], path: '/plan/0/listofcabinettypes' },
  { v: ['PlanView::NewCabinetType'], path: `/plan/0/newcabinettype/${NEW_ID}`, needs: 'ADMIN' },
  { v: ['PlanView::CabinetType'], path: '/plan/0/cabinettype/1' },
  { v: ['PlanView::Leitungskataster'], path: '/plan/0/leitungskataster', needs: 'ADMIN' },
  { v: ['PlanView::Netbox'], path: '/plan/0/netbox', needs: 'ADMIN' },
  { v: ['PlanView::Panel', 'PanelView::Show'], path: '/plan/0/panel/22/show' },
  { v: ['PanelView::Edit'], path: '/plan/1/panel/22/edit', needs: 'PLANNER' },
  { v: ['PanelView::Attach'], path: '/plan/1/panel/22/attach', needs: 'PLANNER' },
  { v: ['PanelView::Loop'], path: '/plan/1/panel/25/loop', needs: 'PLANNER' },
];
// Objects that don't exist: the page shows "<Art> <Id> nicht gefunden" as an alert (Abschnitt 4)
const NOT_FOUND = [
  { path: '/plan/0/cabinet/99999/overview', text: /Schacht 99999 nicht gefunden/ },
  { path: '/plan/0/cable/99999/edit', text: /Kabel 99999 nicht gefunden/ },
  { path: '/plan/0/duct/99999/show', text: /Trasse 99999 nicht gefunden/ },
  { path: '/plan/0/panel/99999/show', text: /Panel 99999 nicht gefunden/ },
  { path: '/plan/99999/edit', text: /Plan(ung)? 99999 nicht gefunden/ },
  { path: '/plan/0/cabinettype/99999', text: /Schachttyp 99999 nicht gefunden/, needs: 'ADMIN' },
];

// A change the server refuses (the mock fails every mutation, MOCK_FAIL): a toast titled
// "<Objekt> konnte nicht … werden", the page stays where it is and keeps the input. `input`
// picks the field to change (index among the text fields), `click` the button, `confirm` the
// button of the dialog asking first.
const FAILING_CHANGES = [
  { path: '/plan/0/duct/711/properties', needs: 'PLANNER', input: 2, click: /^Speichern$/, title: /Trasse konnte nicht gespeichert werden/ },
  { path: '/plan/0/cabinet/1/properties', needs: 'PLANNER', input: 0, click: /^Speichern$/, title: /Schacht konnte nicht gespeichert werden/ },
  { path: '/plan/0/cabinet/1/properties', needs: 'ADMIN', click: /^Löschen$/, confirm: /^(Ja|Löschen)$/, title: /Schacht konnte nicht gelöscht werden/ },
  { path: '/plan/1/edit', needs: 'PLANNER', input: 0, click: /^Umbenennen$/, title: /Plan(ung)? konnte nicht gespeichert werden/ },
  { path: '/plan/0/cabinettype/1', needs: 'ADMIN', input: 0, click: /^Speichern$/, title: /Schachttyp konnte nicht gespeichert werden/ },
  { path: '/plan/1/panel/22/edit', needs: 'PLANNER', input: 0, click: /Speichern$/, title: /(Ports|Panel) konnte(n)? nicht gespeichert werden/ },
  { path: '/plan/0/netbox', needs: 'ADMIN', click: /^Jetzt synchronisieren$/, title: /konnte nicht angestossen werden/ },
  { path: `/plan/0/newcable/${NEW_ID}`, needs: 'PLANNER', input: 0, text: 'Konventionstest', pickDuct: true, click: /^Anlegen$/, title: /Kabel konnte nicht angelegt werden/ },
];

// A change that works: a success toast titled "<Objekt> <Vergangenheit>" (Abschnitt 5) and the
// page the doc says follows (`then`, default: stays); before the change the button is disabled
// (`untouched`: "Speichern" only with changes). Deleting comes last, it changes what the mock has.
const SUCCESSFUL_CHANGES = [
  { path: '/plan/0/duct/711/properties', needs: 'PLANNER', input: 2, click: /^Speichern$/, untouched: true, title: /^Trasse gespeichert$/, then: '/plan/0/duct/711/show' },
  { path: '/plan/0/cabinet/1/properties', needs: 'PLANNER', input: 0, click: /^Speichern$/, untouched: true, title: /^Schacht gespeichert$/, then: '/plan/0/cabinet/1/overview' },
  { path: '/plan/1/edit', needs: 'PLANNER', input: 0, click: /^Umbenennen$/, untouched: true, title: /^Planung umbenannt$/ },
  { path: '/plan/0/cabinettype/1', needs: 'ADMIN', input: 0, click: /^Speichern$/, untouched: true, title: /^Schachttyp gespeichert$/ },
  { path: '/plan/1/panel/22/edit', needs: 'PLANNER', input: 0, click: /^Speichern$/, untouched: true, title: /^Ports gespeichert$/ },
  { path: '/plan/0/netbox', needs: 'ADMIN', click: /^Jetzt synchronisieren$/, title: /^Sync angestossen$/ },
  { path: '/listofplans', needs: 'PLANNER', click: /^Neue Planung$/, dialog: { input: 0, text: 'Konventionstest', click: /^Anlegen$/ }, title: /^Planung angelegt$/ },
  // A new cable needs a duct (its path), its page follows
  { path: `/plan/0/newcable/${NEW_ID}`, needs: 'PLANNER', input: 0, text: 'Konventionstest', pickDuct: true, click: /^Anlegen$/, title: /^Kabel angelegt$/, then: /^\/plan\/0\/cable\/\d+\/edit$/ },
  // Creates a Schachttyp (its page follows) and deletes it again
  { path: `/plan/0/newcabinettype/${NEW_ID}`, needs: 'ADMIN', input: 0, text: 'Konventionstest', click: /^Anlegen$/, title: /^Schachttyp angelegt$/, then: /^\/plan\/0\/cabinettype\/\d+$/,
    next: { click: /^Löschen$/, confirm: /^(Ja|Löschen)$/, title: /^Schachttyp gelöscht$/, then: '/plan/0/listofcabinettypes' } },
  { path: '/plan/0/cable/11/edit', needs: 'ADMIN', click: /^Löschen$/, confirm: /^(Ja|Löschen)$/, title: /^Kabel gelöscht$/, then: '/plan/0/listofcables' },
];
// Pages holding input that isn't stored yet (Abschnitt 5, "Verlassen"): `input` picks the text
// field to change (`text` the new value), `pickDuct` adds a path to a new cable.
const LEAVE_FLOWS = [
  { path: '/plan/0/duct/711/properties', needs: 'PLANNER', input: 2 },
  { path: '/plan/0/cabinet/1/properties', needs: 'PLANNER', input: 0 },
  { path: '/plan/1/edit', needs: 'PLANNER', input: 0 },
  { path: '/plan/0/cabinettype/1', needs: 'ADMIN', input: 0 },
  { path: '/plan/1/panel/22/edit', needs: 'PLANNER', input: 0 },
  { path: `/plan/0/newcable/${NEW_ID}`, needs: 'PLANNER', input: 0, text: 'Konventionstest', pickDuct: true },
];
const RANK = { READER: 0, PLANNER: 1, ADMIN: 2 };
const allowed = (route) => RANK[ROLE] >= RANK[route.needs ?? 'READER'];

const failures = [];
const pending = new Map();
let exceptions = 0;
// `step` unset: a check that is switched on
function report(id, step, where, message) {
  if (step) {
    pending.set(step, (pending.get(step) ?? 0) + 1);
    if (process.env.VERBOSE) console.log(`ausstehend (Schritt ${step}): ${where} ${id} ${message}`);
  } else {
    failures.push(`${where} ${id} ${message}`);
  }
}

// ---------------------------------------------------------------- coverage of the routes
const router = fs.readFileSync(path.join(HERE, '../../cable-editor-frontend/src/pages/router.rs'), 'utf8');
const variants = ['AppRoute', 'PlanView', 'CabinetView', 'CableView', 'DuctView', 'PanelView'].flatMap((name) => {
  const body = router.match(new RegExp(`pub enum ${name} \\{([\\s\\S]*?)\\n\\}`))?.[1] ?? '';
  const list = [...body.matchAll(/^    ([A-Z]\w*)/gm)].map((m) => `${name}::${m[1]}`);
  if (!list.length) failures.push(`router.rs: enum ${name} nicht gefunden (Prüfung anpassen)`);
  return list;
});
const covered = new Set(ROUTES.flatMap((r) => r.v));
for (const v of variants) {
  if (!covered.has(v)) failures.push(`local/konventionen/pages.mjs: keine Route für ${v} (in ROUTES eintragen)`);
}
for (const v of covered) {
  if (!variants.includes(v)) failures.push(`local/konventionen/pages.mjs: ${v} gibt es nicht mehr (aus ROUTES entfernen)`);
}

// ---------------------------------------------------------------- checks of a page
// Every visible button and link must have a name for screen readers; a symbol without text also
// needs a title (tooltip for everyone else)
const inspect = () => {
  const visible = (el) => {
    const r = el.getBoundingClientRect();
    const s = getComputedStyle(el);
    return r.width > 0 && r.height > 0 && s.visibility !== 'hidden' && s.display !== 'none';
  };
  const name = (el) =>
    (el.getAttribute('aria-label') ?? '') || (el.innerText ?? '').trim() || (el.getAttribute('title') ?? '') ||
    [...el.querySelectorAll('img[alt]')].map((i) => i.alt).join('').trim();
  const describe = (el) => {
    const icon = el.querySelector('i, svg, img');
    return `<${el.tagName.toLowerCase()} class="${el.className}"> ${icon ? `Symbol ${icon.getAttribute('class') ?? icon.tagName.toLowerCase()}` : 'ohne Symbol'}, in „${(el.closest('tr, li, section, form')?.innerText ?? '').trim().replace(/\s+/g, ' ').slice(0, 40)}“`;
  };
  // Leaflet's markers are labelled by their tooltip and belong to the map, not to the page's controls
  const controls = [...document.querySelectorAll('button, a[href], [role=button]')]
    .filter((el) => visible(el) && !el.classList.contains('leaflet-marker-icon'));
  return {
    h1: [...document.querySelectorAll('h1')].filter(visible).map((h) => h.innerText.trim()),
    overflow: document.documentElement.scrollWidth - window.innerWidth,
    unnamed: controls.filter((el) => !name(el)).map(describe),
    symbolWithoutTitle: controls
      .filter((el) => !(el.innerText ?? '').trim() && el.querySelector('svg, img, i') && name(el) && !(el.getAttribute('aria-label') && el.getAttribute('title')))
      .map(describe),
    buttonTexts: controls.filter((el) => el.tagName === 'BUTTON').map((el) => (el.innerText ?? '').trim()).filter(Boolean),
  };
};

const FORBIDDEN_BUTTONS = /^(Ja|Nein|OK|Drucken|Drucken \/ PDF|Änderungen Speichern|Aktivieren|Planung eröffnen|Neue Planung erstellen)$/;

// The menus of the breadcrumb bar: each offers one level, with exactly one entry selected
async function checkBreadcrumb(page, where, route) {
  const toggles = page.locator('.breadcrumb-bar .popup-menu > button, .breadcrumb-bar .popup-menu .pf-v6-c-menu-toggle');
  const count = await toggles.count();
  for (let i = 0; i < count; i++) {
    const toggle = toggles.nth(i);
    const label = ((await toggle.getAttribute('aria-label')) ?? (await toggle.innerText())).trim();
    await toggle.click();
    await page.waitForTimeout(150);
    const menu = page.locator('.breadcrumb-bar .pf-v6-c-menu:visible').first();
    if (await menu.count()) {
      const isUserMenu = (await menu.locator('.user-menu__entry').count()) > 0;
      const selected = await menu.locator('.pf-m-selected').count();
      // The properties of an object open from a button, not from the menu (Abschnitt 2)
      const none = selected === 0 && allowed(route) && !route.path.endsWith('/properties');
      if (!isUserMenu && (selected > 1 || none)) {
        report('breadcrumb-ein-aktiver-eintrag', undefined, where, `Menü „${label}“ hat ${selected} aktive Einträge (erwartet 1)`);
      }
    }
    await page.keyboard.press('Escape');
    await page.mouse.click(1, 1);
    await page.waitForTimeout(100);
  }
}

async function checkRoute(browser, device, deviceName, route) {
  const where = `${route.path} (${deviceName}, ${ROLE})`;
  const context = await browser.newContext(device);
  await context.addInitScript(() => {
    if (!navigator.bluetooth) Object.defineProperty(navigator, 'bluetooth', { value: { getAvailability: async () => true } });
  });
  const page = await context.newPage();
  const errors = [];
  page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));
  page.on('pageerror', (e) => errors.push(e.message));
  try {
    await page.goto(ORIGIN + route.path);
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);
    const found = await page.evaluate(inspect);

    for (const e of errors) report('konsolenfehler', undefined, where, e.slice(0, 200));
    if (found.overflow > 0) report('ueberlauf', undefined, where, `Seite ${found.overflow}px breiter als das Gerät`);
    if (found.h1.length !== 1) report('ein-h1', undefined, where, `${found.h1.length} sichtbare h1: ${JSON.stringify(found.h1)}`);
    if (!allowed(route)) {
      if (!found.h1.includes('Keine Berechtigung')) report('rolle', undefined, where, `sollte „Keine Berechtigung“ zeigen, zeigt ${JSON.stringify(found.h1)}`);
    }
    for (const el of found.unnamed) report('steuerelement-ohne-name', undefined, where, `${el} hat keinen Namen`);
    for (const el of found.symbolWithoutTitle) report('symbolknopf-ohne-name', undefined, where, `${el} braucht aria-label und title`);
    for (const text of new Set(found.buttonTexts)) {
      if (FORBIDDEN_BUTTONS.test(text)) report('verboten-beschriftung', undefined, where, `Knopf „${text}“ heisst nicht so (Abschnitt 1 der Doku)`);
    }
    await checkBreadcrumb(page, where, route);
  } catch (e) {
    report('seite-nicht-pruefbar', undefined, where, String(e).split('\n')[0]);
  } finally {
    await context.close();
  }
}

async function withPage(browser, fn) {
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  const page = await context.newPage();
  try {
    await fn(page);
  } catch (e) {
    report('seite-nicht-pruefbar', undefined, page.url(), String(e).split('\n')[0]);
  } finally {
    await context.close();
  }
}

async function checkNotFound(browser, entry) {
  const where = `${entry.path} (Desktop, ${ROLE})`;
  if (RANK[ROLE] < RANK[entry.needs ?? 'READER']) return;
  await withPage(browser, async (page) => {
    await page.goto(ORIGIN + entry.path);
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(800);
    const alerts = await page.locator('main .pf-v6-c-alert.pf-m-danger').allInnerTexts();
    if (!alerts.some((a) => entry.text.test(a))) {
      report('nicht-gefunden', undefined, where, `sollte ${entry.text} als Alert zeigen, zeigt ${JSON.stringify(alerts)}`);
    }
  });
}

// The path of a new cable: the first duct of the dialog "Trasse auswählen"
async function pickDuct(page) {
  await page.locator('main button:visible', { hasText: /^Trasse auswählen$/ }).first().click();
  await page.locator('.pf-v6-c-modal-box tbody tr').first().click();
  await page.waitForTimeout(500);
}

async function checkFailingChange(browser, entry) {
  const where = `${entry.path} „${entry.click.source}“ (Desktop, ${ROLE})`;
  if (RANK[ROLE] < RANK[entry.needs ?? 'READER']) return;
  await withPage(browser, async (page) => {
    await page.goto(ORIGIN + entry.path);
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(800);
    let typed;
    if (entry.input !== undefined) {
      const field = page.locator('main input[type=text]:visible').nth(entry.input);
      typed = entry.text ?? `${await field.inputValue()} x`;
      await field.fill(typed);
    }
    if (entry.pickDuct) await pickDuct(page);
    await page.locator('main button:visible', { hasText: entry.click }).first().click();
    if (entry.confirm) {
      await page.locator('.pf-v6-c-modal-box button', { hasText: entry.confirm }).first().click();
    }
    await page.waitForTimeout(800);
    const toasts = await page.locator('.pf-v6-c-alert-group .pf-v6-c-alert.pf-m-danger').allInnerTexts();
    if (!toasts.some((t) => entry.title.test(t) && TOAST_TITLE.test(t))) {
      report('fehler-als-toast', undefined, where, `sollte einen Toast ${entry.title} zeigen, zeigt ${JSON.stringify(toasts)}`);
    }
    if (new URL(page.url()).pathname !== entry.path) report('fehler-als-toast', undefined, where, `hat die Seite verlassen: ${page.url()}`);
    if (typed !== undefined) {
      const kept = await page.locator('main input[type=text]:visible').nth(entry.input).inputValue();
      if (kept !== typed) report('fehler-als-toast', undefined, where, `Eingabe „${typed}“ ging verloren (${JSON.stringify(kept)})`);
    }
  });
}

async function checkSuccessfulChange(browser, entry) {
  if (RANK[ROLE] < RANK[entry.needs ?? 'READER']) return;
  await withPage(browser, async (page) => {
    await page.goto(ORIGIN + entry.path);
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(800);
    for (let step = entry; step; step = step.next) {
      const where = `${page.url().replace(ORIGIN, '')} „${step.click.source}“ (Desktop, ${ROLE})`;
      const button = page.locator('main button:visible', { hasText: step.click }).first();
      if (step.untouched && !(await button.isDisabled())) {
        report('speichern-ohne-aenderung', undefined, where, 'der Knopf ist ohne Änderung aktiv');
      }
      if (step.input !== undefined) {
        const field = page.locator('main input[type=text]:visible').nth(step.input);
        await field.fill(step.text ?? `${await field.inputValue()} x`);
      }
      if (step.pickDuct) await pickDuct(page);
      await button.click();
      if (step.dialog) {
        await page.locator('.pf-v6-c-modal-box input[type=text]:visible').nth(step.dialog.input).fill(step.dialog.text);
        await page.locator('.pf-v6-c-modal-box button', { hasText: step.dialog.click }).first().click();
      }
      if (step.confirm) {
        await page.locator('.pf-v6-c-modal-box button', { hasText: step.confirm }).first().click();
      }
      await page.waitForTimeout(800);
      const toasts = await page.locator('.pf-v6-c-alert-group .pf-v6-c-alert.pf-m-success').allInnerTexts();
      if (!toasts.some((t) => t.split('\n').some((line) => step.title.test(line.trim())))) {
        report('toast-nach-aenderung', undefined, where, `sollte einen Erfolgs-Toast ${step.title} zeigen, zeigt ${JSON.stringify(toasts)}`);
      }
      const now = new URL(page.url()).pathname;
      const expected = step.then ?? step.path ?? entry.path;
      if (!(expected instanceof RegExp ? expected.test(now) : now === expected)) {
        report('wohin-danach', undefined, where, `sollte auf ${expected} stehen, steht auf ${now}`);
      }
      // Stored changes aren't lost by leaving: nothing is asked
      if (!step.next && !step.then && step.input !== undefined) {
        const link = await leaveLink(page);
        if (link) {
          await link.click();
          await page.waitForTimeout(500);
          if (await dialogOf(page).count()) report('aenderungen-verwerfen', undefined, where, 'fragt nach dem Speichern noch nach');
        }
      }
    }
  });
}


// ---------------------------------------------------------------- leaving a page with changes
const DISCARD_TITLE = 'Änderungen verwerfen?';

// A link to another page of the app: one on the page, else the first entry of a breadcrumb menu
// (the menu stays open). `null` if there is none.
async function leaveLink(page) {
  const find = () => page.evaluate(() => {
    const here = location.pathname;
    const link = [...document.querySelectorAll('a[href]')].find((a) => {
      const r = a.getBoundingClientRect();
      const url = new URL(a.href);
      return r.width > 0 && r.height > 0 && url.origin === location.origin && url.pathname !== here && a.target !== '_blank' && !a.hasAttribute('download');
    });
    return link?.getAttribute('href') ?? null;
  });
  let href = await find();
  const toggles = page.locator('main nav button:visible');
  for (let i = 0; !href && i < (await toggles.count()); i++) {
    await toggles.nth(i).click();
    await page.waitForTimeout(200);
    href = await find();
    if (!href) await page.keyboard.press('Escape');
  }
  return href ? page.locator(`a[href="${href}"]:visible`).first() : null;
}

// Opens `path` the way the router does, so the browser has a page to go back to
async function openInApp(page, path) {
  await page.evaluate((path) => {
    history.pushState(null, '', path);
    window.dispatchEvent(new PopStateEvent('popstate'));
  }, path);
  await page.waitForLoadState('networkidle');
  await page.waitForTimeout(800);
}

async function change(page, entry) {
  const field = page.locator('main input[type=text]:visible').nth(entry.input);
  await field.fill(entry.text ?? `${await field.inputValue()} x`);
  if (entry.pickDuct) await pickDuct(page);
  await page.waitForTimeout(300);
  return field;
}

const pathOf = (page) => new URL(page.url()).pathname;
const dialogOf = (page) => page.locator('.pf-v6-c-modal-box', { hasText: DISCARD_TITLE });

async function expectDialog(page, where, what) {
  const shown = await dialogOf(page).count();
  if (!shown) report('aenderungen-verwerfen', undefined, where, `${what}: sollte „${DISCARD_TITLE}“ fragen, fragt nicht`);
  return shown > 0;
}

async function checkLeaveFlow(browser, entry) {
  if (RANK[ROLE] < RANK[entry.needs ?? 'READER']) return;
  const where = `${entry.path} (Desktop, ${ROLE})`;
  // Without changes nothing is asked
  await withPage(browser, async (page) => {
    await page.goto(ORIGIN + entry.path);
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(800);
    const link = await leaveLink(page);
    if (!link) return report('aenderungen-verwerfen', undefined, where, 'keine Verknüpfung zu einer anderen Seite gefunden (Prüfung anpassen)');
    await link.click();
    await page.waitForTimeout(500);
    if (await dialogOf(page).count()) report('aenderungen-verwerfen', undefined, where, 'fragt ohne Änderungen nach');
    if (pathOf(page) === entry.path) report('aenderungen-verwerfen', undefined, where, 'Verknüpfung ohne Änderungen führt nicht weiter');
  });
  // Link: asks, "Weiter bearbeiten" stays, "Verwerfen" leaves
  await withPage(browser, async (page) => {
    await page.goto(ORIGIN + entry.path);
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(800);
    const field = await change(page, entry);
    const typed = await field.inputValue();
    let link = await leaveLink(page);
    if (!link) return;
    await link.click();
    await page.waitForTimeout(500);
    if (!(await expectDialog(page, where, 'Verknüpfung'))) return;
    if (await page.locator('main nav button[aria-expanded=true]').count()) report('aenderungen-verwerfen', undefined, where, 'Verknüpfung: das Menü bleibt hinter dem Dialog offen');
    if (pathOf(page) !== entry.path) report('aenderungen-verwerfen', undefined, where, `Verknüpfung: hat die Seite schon vor der Antwort verlassen: ${pathOf(page)}`);
    await page.locator('.pf-v6-c-modal-box button', { hasText: /^Weiter bearbeiten$/ }).click();
    await page.waitForTimeout(500);
    if (pathOf(page) !== entry.path) report('aenderungen-verwerfen', undefined, where, `„Weiter bearbeiten“ hat die Seite verlassen: ${pathOf(page)}`);
    if (await dialogOf(page).count()) report('aenderungen-verwerfen', undefined, where, '„Weiter bearbeiten“ schliesst den Dialog nicht');
    if ((await field.inputValue()) !== typed) report('aenderungen-verwerfen', undefined, where, '„Weiter bearbeiten“ hat die Eingabe verworfen');
    link = await leaveLink(page);
    await link.click();
    await page.waitForTimeout(500);
    if (!(await expectDialog(page, where, 'Verknüpfung, zweites Mal'))) return;
    await page.locator('.pf-v6-c-modal-box button', { hasText: /^Verwerfen$/ }).click();
    await page.waitForTimeout(800);
    if (pathOf(page) === entry.path) report('aenderungen-verwerfen', undefined, where, '„Verwerfen“ führt nicht weiter');
    if (await dialogOf(page).count()) report('aenderungen-verwerfen', undefined, where, '„Verwerfen“ schliesst den Dialog nicht');
  });
  // "Zurück" in the browser: the address stays, the question comes
  await withPage(browser, async (page) => {
    await page.goto(ORIGIN + '/listofplans');
    await page.waitForLoadState('networkidle');
    await openInApp(page, entry.path);
    await change(page, entry);
    await page.goBack();
    await page.waitForTimeout(800);
    if (pathOf(page) !== entry.path) report('aenderungen-verwerfen', undefined, where, `„Zurück“: die Adresse wechselt auf ${pathOf(page)}`);
    if (!(await expectDialog(page, where, '„Zurück“'))) return;
    await page.locator('.pf-v6-c-modal-box button', { hasText: /^Weiter bearbeiten$/ }).click();
    await page.waitForTimeout(500);
    if (pathOf(page) !== entry.path) report('aenderungen-verwerfen', undefined, where, `„Zurück“, „Weiter bearbeiten“: steht auf ${pathOf(page)}`);
    await page.goBack();
    await page.waitForTimeout(800);
    if (!(await expectDialog(page, where, '„Zurück“, zweites Mal'))) return;
    await page.locator('.pf-v6-c-modal-box button', { hasText: /^Verwerfen$/ }).click();
    await page.waitForTimeout(800);
    if (pathOf(page) !== '/listofplans') report('aenderungen-verwerfen', undefined, where, `„Zurück“, „Verwerfen“: steht auf ${pathOf(page)}`);
  });
  // Closing the window: the browser's warning
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  try {
    const page = await context.newPage();
    const kinds = [];
    page.on('dialog', (d) => {
      kinds.push(d.type());
      d.dismiss().catch(() => {});
    });
    await page.goto(ORIGIN + entry.path);
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(800);
    await change(page, entry);
    await page.locator('main').click({ position: { x: 2, y: 2 } });
    await page.close({ runBeforeUnload: true });
    await page.waitForTimeout(500);
    if (!kinds.includes('beforeunload')) report('aenderungen-verwerfen', undefined, where, 'Fenster schliessen: keine Warnung des Browsers');
  } catch (e) {
    report('seite-nicht-pruefbar', undefined, where, String(e).split('\n')[0]);
  } finally {
    await context.close();
  }
}

// ---------------------------------------------------------------- run
const browser = await chromium.launch();
try {
  const check = await fetch(`${ORIGIN}/listofplans`).catch(() => null);
  if (!check?.ok) throw new Error(`Mock unter ${ORIGIN} nicht erreichbar (local/konventionen/run.sh startet ihn)`);
  for (const [deviceName, device] of [['Handy', devices['Pixel 7']], ['Desktop', { viewport: { width: 1440, height: 900 } }]]) {
    for (const route of ROUTES) await checkRoute(browser, device, deviceName, route);
  }
  for (const entry of NOT_FOUND) await checkNotFound(browser, entry);
  for (const entry of LEAVE_FLOWS) await checkLeaveFlow(browser, entry);
  for (const entry of SUCCESSFUL_CHANGES) await checkSuccessfulChange(browser, entry);
  await fetch(`${ORIGIN}/mock/fail?mutations=*`);
  try {
    for (const entry of FAILING_CHANGES) await checkFailingChange(browser, entry);
  } finally {
    await fetch(`${ORIGIN}/mock/fail?mutations=`);
  }
} finally {
  await browser.close();
}

const unique = [...new Set(failures)];
for (const f of unique) console.log(f);
const steps = [...pending].sort((a, b) => a[0] - b[0]).map(([step, n]) => `Schritt ${step}: ${n}`).join(', ');
console.log(`Seiten (${ROLE}): ${unique.length} Verstösse, ${[...pending.values()].reduce((a, b) => a + b, 0)} ausstehend${steps ? ` (${steps})` : ''}, ${exceptions} Ausnahmen`);
process.exit(unique.length ? 1 : 0);
