// Static check of the frontend sources against docs/frontend-konventionen.md.
//
//   node local/konventionen/static.mjs
//
// Prints `file:line rule message` for every hit, exit code 1 if a rule that is switched on
// (`step` unset) has hits. A rule with `step: N` is not implemented yet: its hits are only counted
// ("pending"), the commit of step N of the doc's "Umsetzung" removes the `step`. An exception is
// a comment `// konventionen:ignore <rule> <reason>` in the line before the hit.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.join(path.dirname(fileURLToPath(import.meta.url)), '../../cable-editor-frontend/src');

const walk = (dir) =>
  fs.readdirSync(dir, { withFileTypes: true }).flatMap((e) =>
    e.isDirectory() ? walk(path.join(dir, e.name)) : e.name.endsWith('.rs') ? [path.join(dir, e.name)] : []);

const files = walk(ROOT).map((file) => {
  const text = fs.readFileSync(file, 'utf8');
  return { file, rel: path.relative(ROOT, file), text, lines: text.split('\n') };
});

const isComment = (line) => /^\s*\/\//.test(line);
// Line numbers (0 based) of the lines matching `pattern`, comments left out
const grep = ({ lines }, pattern) =>
  lines.flatMap((line, i) => (!isComment(line) && pattern.test(line) ? [i] : []));

// The opening tags `<Button ...>` with everything up to the end of the element: [line, tag text,
// whether it is self-closing, the children]
function buttons({ text }) {
  const found = [];
  for (const match of text.matchAll(/<Button\b/g)) {
    let depth = 0;
    let i = match.index + match[0].length;
    for (; i < text.length; i++) {
      const c = text[i];
      if (c === '{') depth++;
      else if (c === '}') depth--;
      else if (c === '>' && depth === 0) break;
    }
    const tag = text.slice(match.index, i + 1);
    const selfClosing = text[i - 1] === '/';
    const end = selfClosing ? -1 : text.indexOf('</Button>', i);
    const children = end < 0 ? '' : text.slice(i + 1, end);
    found.push({ line: text.slice(0, match.index).split('\n').length - 1, tag, selfClosing, children });
  }
  return found;
}

const FORBIDDEN_LABELS = [
  ['Änderungen Speichern', 'Speichern'],
  ['Loops Verbinden', 'Loops verbinden'],
  ['Ports ändern', 'Ports bearbeiten'],
  ['Ja', 'Löschen (Bestätigungsdialog, components/dialog.rs)'],
  ['Nein', 'Abbrechen'],
  ['Drucken / PDF', 'Seite drucken'],
  ['Drucken', 'Etikett drucken oder Seite drucken'],
  ['Drucker verbinden', 'Etikettendrucker verbinden'],
  ['Drucker trennen', 'Etikettendrucker trennen'],
  ['Planung eröffnen', 'Anlegen'],
  ['Neue Planung erstellen', 'Neue Planung'],
  ['Aktivieren', 'In Netbox aktivieren'],
];
const TOAST_TITLE = /konnte(n)? nicht (gespeichert|angelegt|gelöscht|geladen|angestossen|geändert|abgeschlossen|aktiviert) werden/;

// Erfolgs-Toast: "Schacht gespeichert", "In Netbox aktiviert", "Sync angestossen", "Standard-Eigentümer gesetzt"
const SUCCESS_TITLE = /^[\p{L}\d -]+ (gespeichert|angelegt|gelöscht|geändert|angestossen|abgeschlossen|aktiviert|umbenannt|gesetzt|geliefert|zurückgenommen|bestätigt)$/u;

// Functions of graphql/ that send a mutation: what pages and components call to change stored
// data, as the regular expression of their call (`Type::name(`, `.name(` for methods, `name(`)
const MUTATIONS = files
  .filter((f) => f.rel.startsWith('graphql'))
  .flatMap((f) => {
    const calls = [];
    let type;
    let current;
    for (const line of f.lines) {
      const impl = line.match(/^impl(?:<[^>]*>)? (?:\w+ for )?(\w+)/);
      if (impl) type = impl[1];
      else if (/^\S/.test(line) && !/^impl/.test(line) && !/^[}\/#]/.test(line)) type = undefined;
      const fn = line.match(/\bfn (\w+)(?:<[^>]*>)?\(([^)]*)/);
      if (fn) current = { name: fn[1], type, self: /\bself\b/.test(fn[2]) };
      if (/\bmutate::</.test(line) && current) {
        const { name, type, self } = current;
        calls.push(type ? (self ? new RegExp(`\\.${name}\\(`) : new RegExp(`\\b${type}::${name}\\(`)) : new RegExp(`(?<![.\\w:])${name}\\(`));
      }
    }
    return calls;
  });

const rules = [
  {
    id: 'verboten-beschriftung',
    message: 'Beschriftung nach Abschnitt 1 der Doku ändern',
    find: (f) =>
      FORBIDDEN_LABELS.flatMap(([bad, good]) =>
        grep(f, new RegExp(`"${bad}"`)).map((line) => ({ line, note: `„${bad}“ heisst „${good}“` }))),
  },
  {
    id: 'plan-statt-planung',
    step: 5,
    message: 'in der Oberfläche heisst der Plan „Planung“',
    find: (f) => grep(f, /"[^"]*\bPlan\b[^"]*"/).filter((line) => !/^\s*(use|mod|#\[)/.test(f.lines[line])).map((line) => ({ line })),
  },
  {
    id: 'fehler-als-text',
    message: 'Fehler als FrontendError zeigen (Alert/Toast), nicht als to_string() oder {:?}',
    find: (f) =>
      grep(f, /Alert\b[^\n]*title=\{[^}]*(\.to_string\(\)|:\?)/)
        .concat(grep(f, /toast_(error|success)\([^\n]*(\.to_string\(\)|\{[a-z_]*:\?\})/))
        .map((line) => ({ line })),
  },
  {
    id: 'nicht-gefunden-eigener-text',
    message: '„nicht gefunden“ ist immer FrontendError::NotFound (error/messages.rs)',
    find: (f) => (f.rel === path.join('error', 'messages.rs') ? [] : grep(f, /"[^"]*nicht gefunden[^"]*"/i).map((line) => ({ line }))),
  },
  {
    id: 'toast-titel',
    message: 'Titel eines Fehler-Toasts: „<Objekt> konnte nicht gespeichert|angelegt|gelöscht|geladen|angestossen|geändert|abgeschlossen|aktiviert werden“',
    find: (f) => {
      const hits = [];
      for (const match of f.text.matchAll(/toast_error\(([^;]*?)\)(?:;|,\s*\n|\s*\n\s*[}\)])/gs)) {
        const title = match[1].match(/"([^"]*)"/)?.[1];
        if (title !== undefined && !TOAST_TITLE.test(title)) {
          hits.push({ line: f.text.slice(0, match.index).split('\n').length - 1, note: `„${title}“` });
        }
      }
      return hits;
    },
  },
  {
    id: 'symbolknopf-ohne-name',
    message: 'Knopf nur mit Symbol: IconButton (components/icon_button.rs) mit name statt Button, der kein title kennt',
    find: (f) =>
      buttons(f)
        .filter((b) => /\bicon=/.test(b.tag) && !/\blabel=/.test(b.tag) && !b.children.trim())
        .map((b) => ({ line: b.line })),
  },
  {
    id: 'print-nur-ein-ort',
    message: 'Seite drucken (window().print()) nur im Knopf „Seite drucken“ (components/print_page.rs)',
    find: (f) => (f.rel === path.join('components', 'print_page.rs') ? [] : grep(f, /\.print\(\)/).map((line) => ({ line }))),
  },
  {
    id: 'aenderung-ohne-toast',
    message: 'Wer eine Änderung an gespeicherten Daten abschickt, bestätigt sie mit util::toast_success (Abschnitt 5)',
    find: (f) => {
      if (f.rel.startsWith('graphql') || /toast_success/.test(f.text)) return [];
      return f.lines.flatMap((line, i) => {
        if (isComment(line) || /\bfn \w+/.test(line)) return [];
        const call = MUTATIONS.find((pattern) => pattern.test(line));
        return call ? [{ line: i, note: 'toast_success fehlt in dieser Datei' }] : [];
      });
    },
  },
  {
    id: 'toast-titel-erfolg',
    message: 'Titel eines Erfolgs-Toasts: „<Objekt> <Vergangenheit>“, z. B. „Schacht gespeichert“',
    find: (f) =>
      f.lines.flatMap((line, i) => {
        const title = line.match(/toast_success\([^"]*"([^"]*)"/)?.[1];
        return title !== undefined && !SUCCESS_TITLE.test(title) ? [{ line: i, note: `„${title}“` }] : [];
      }),
  },
  {
    id: 'speichern-ohne-aenderung',
    message: '„Speichern“ und „Anlegen“ brauchen disabled: aktiv nur mit gültigen Änderungen (Abschnitt 5)',
    find: (f) =>
      buttons(f)
        .filter((b) => /\blabel=(\{[^}]*)?"(Speichern|Anlegen)"/.test(b.tag) && !/(\bdisabled=|\{disabled\})/.test(b.tag))
        .map((b) => ({ line: b.line })),
  },
  {
    id: 'roher-link',
    step: 4,
    message: 'PlanLink statt Link<AppRoute> verwenden',
    find: (f) => (f.rel === path.join('components', 'plan_link.rs') ? [] : grep(f, /<Link<AppRoute>/).map((line) => ({ line }))),
  },
];

const ignored = (f, line, id) => new RegExp(`konventionen:ignore\\s+${id}\\s+\\S`).test(f.lines[line - 1] ?? '');

let failed = 0;
let pending = 0;
const pendingByStep = new Map();
for (const rule of rules) {
  for (const f of files) {
    for (const hit of rule.find(f)) {
      if (ignored(f, hit.line, rule.id)) continue;
      const text = `${f.rel}:${hit.line + 1} ${rule.id} ${rule.message}${hit.note ? ` (${hit.note})` : ''}`;
      if (rule.step) {
        pending++;
        pendingByStep.set(rule.step, (pendingByStep.get(rule.step) ?? 0) + 1);
        if (process.env.VERBOSE) console.log(`ausstehend (Schritt ${rule.step}): ${text}`);
      } else {
        failed++;
        console.log(text);
      }
    }
  }
}

const exceptions = files.reduce(
  (n, f) => n + f.lines.filter((l) => /konventionen:ignore\s+\S+\s+\S|#\[allow\(clippy::(expect_used|unwrap_used|panic|disallowed_[a-z]+)/.test(l)).length, 0);
const steps = [...pendingByStep].sort((a, b) => a[0] - b[0]).map(([step, n]) => `Schritt ${step}: ${n}`).join(', ');
console.log(`statisch: ${failed} Verstösse, ${pending} ausstehend${steps ? ` (${steps})` : ''}, ${exceptions} Ausnahmen`);
process.exit(failed ? 1 : 0);
