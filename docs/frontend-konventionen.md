# Konzept: Einheitliche Bedienung im Frontend

Stand: 30.09.2026 – offene Fragen entschieden; das Prüfgerüst steht, alle Schritte unter „Umsetzung“ sind umgesetzt.

Diese Regeln gelten für jede neue oder geänderte Seite. Sie stammen aus einer Durchsicht des
ganzen Frontends; wo der Code noch abweicht, steht es unter „Umsetzung“. Wie Fehler entstehen
und was das Backend liefert, steht in `fehlermeldungen.md`, der Aufbau des Breadcrumbs in
`navigation.md`; hier geht es um das, was Benutzer an jeder Seite gleich erwarten.

## 1. Benennung von Aktionen

- Eine Aktion heisst überall gleich. Beschriftungen sind kurze Verben im Infinitiv, nur das
  erste Wort gross („Speichern“, nicht „Änderungen Speichern“).
- Feste Beschriftungen:

  | Aktion | Beschriftung | Variante |
  | --- | --- | --- |
  | Änderungen eines Formulars oder Editors übernehmen | „Speichern“ | Primary |
  | neues Objekt im Formular übernehmen | „Anlegen“ | Primary |
  | Formular oder Dialog verlassen, ohne zu übernehmen | „Abbrechen“ | Link |
  | gespeichertes Objekt löschen | „Löschen“ | DangerSecondary |
  | Teil eines Objekts aus dem Editor nehmen (Segment, Port, Panel), gespeichert erst mit „Speichern“ | „Entfernen“ | Secondary |
  | neues Objekt auf einer Liste | „Neuer Schacht“, „Neue Trasse“, „Neues Kabel“, „Neuer Eigentümer“, „Neuer Schachttyp“, „Neue Planung“ | Primary |
  | Formular eines Objekts öffnen | „Bearbeiten“ | Secondary |
  | nach einem Fehler nochmals laden | „Erneut laden“ | |

- Beschriftung, Titel der Ansicht und Breadcrumb-Eintrag sind derselbe Text, wenn sie dasselbe
  meinen („Loops verbinden“ überall so geschrieben). Ansichten zum Ändern heissen „… bearbeiten“,
  nicht „… ändern“.
- Ein Bestätigungsdialog wiederholt das Verb der Aktion: Titel „<Objekt> löschen?“, der Text
  nennt das Objekt und was daran hängt, die Knöpfe heissen „Löschen“ (Danger) und
  „Abbrechen“ (Link); nicht „Ja“ und „Nein“. Bei „Planung abschliessen“ heisst der Knopf
  „Abschliessen“, bei „In Netbox aktivieren“ „In Netbox aktivieren“.
- „Löschen“ entfernt ein gespeichertes Objekt sofort, „Entfernen“ nimmt etwas aus einem Editor,
  dessen Änderungen noch nicht gespeichert sind.
- Knöpfe nur mit Symbol haben `aria_label` und `title` mit dem Namen der Aktion, denn auf
  Touchgeräten gibt es keinen Tooltip und Screenreader brauchen den Namen.
- Feldbeschriftungen sind Substantive, gross geschrieben („Name“).
- **Drucken:** Es gibt zwei verschiedene Dinge, und der Name sagt, welches:
  - „Etikett drucken“ druckt ein Etikett auf dem Etikettendrucker (Brady, per Bluetooth). Dazu
    gehören der Dialog („Etikett drucken“, Knopf „Etikett drucken“), die Statusleiste
    („Etikettendrucker verbinden“, „Etikettendrucker trennen“, „Kein Etikettendrucker
    verbunden“) und der Knopf, überall „Etikett drucken“, auch in Tabellenzeilen.
  - „Seite drucken“ öffnet den Druckdialog des Geräts für die ganze Seite (Drucker oder PDF,
    `window.print()`); Symbol Drucker, `title` „Druckdialog des Geräts öffnen (Drucker oder PDF)“.
  - Beide Knöpfe tragen unterschiedliche Symbole, damit sie auch ohne Text auseinanderzuhalten
    sind.
  - Die Knöpfe für Etiketten erscheinen nur, wenn der Browser Web Bluetooth kann
    (`use_printer_supported`, `check_printer_supported`), ebenso die Statusleiste. Ob ein
    Drucker am Gerät eingerichtet ist, kann ein Browser nicht erkennen; „Seite drucken“ bleibt
    darum immer sichtbar, denn der Druckdialog bietet auch PDF an.
- Ein Plan heisst in der Oberfläche überall „Planung“ („Planung: Ist-Zustand“, „Planung
  bearbeiten“, „Neue Planung“); der Ist-Zustand ist die Planung 0. Im Code bleibt es `plan`.

## 2. Menüführung

- Jede Art von Objekt ist ein Bereich im Bereichsmenü. Die Bereiche sind nach Zweck gruppiert:
  Planung (Planung bearbeiten, Karte), Objekte (Schacht, Kabel, Trasse), Stammdaten (Eigentümer,
  Schachttyp), Lieferung (Leitungskataster, Netbox).
- Im Menü einer Ebene stehen die Ansichten, mit denen man am Objekt arbeitet (Übersicht,
  Panels bearbeiten, Fasern auflegen). „Bearbeiten“ der Eigenschaften eines Objekts steht nicht
  im Menü, sondern als Knopf auf der Übersicht des Objekts, sichtbar ab der nötigen Rolle.
  Ausnahme ist das Kabel: Es hat nur eine Seite, die die Einstellungen zeigt und, mit der nötigen
  Rolle, ändern lässt (für Leser schreibgeschützt); dort gibt es keinen Knopf „Bearbeiten“.
- Ein neues Objekt zeigt im Breadcrumb als letztes Element „Neuer Schacht“ usw., als Text, für
  alle Arten gleich.
- Was wie ein Knopf aussieht und navigiert, ist ein `PlanLink` mit Knopf-Klassen; Seiten bauen
  kein rohes `Link<AppRoute>`.
- Neue Objekte anlegen: Objekte mit vielen Feldern, Karte oder Datei (Schacht, Trasse, Kabel,
  Schachttyp) bekommen keine eigene Seite zum Anlegen, sondern öffnen dieselbe Seite wie zum
  Bearbeiten, mit `IdOrNew` als Schlüssel (`New…`); was sich beim Anlegen unterscheidet (Beschriftung
  „Anlegen“, Vorgaben), unterscheidet die Komponente selbst. Objekte mit ein bis zwei Feldern
  (Eigentümer, Planung) bekommen einen Dialog. Nach dem Anlegen steht das Objekt in der Liste bzw.
  auf seiner Seite.

## 3. Verlinkung zwischen den Pages

- Wo eine Seite (ausser Formularen) ein Objekt nennt, ist der Name ein Link auf dessen
  Übersicht, mit der Komponente aus `components/links.rs`; dasselbe Objekt führt überall zum
  selben Ziel und trägt überall denselben Namen (z. B. `duct_title` bei Trassen ohne
  Beschreibung).
- Das gilt für Schacht, Panel, Kabel, Trasse, Schachttyp und Plan. Der Eigentümer hat keine
  eigene Seite und verlinkt auf die Liste der Eigentümer.
- Auf Formularseiten führen Links nicht von der Seite weg, solange Eingaben ungespeichert sind
  (siehe 5); bei Karten dort keine Navigation aus der Karte.

## 4. Fehlerbehandlung

- Ein Fehler erreicht Benutzer immer als `FrontendError` mit seinem Titel, den Details und der
  Herkunft; nie als `to_string()` oder `{:?}` einer Library. Fehler des Browsers (Datei lesen,
  Standort, Canvas) werden über einen Helfer zu einem `FrontendError`; im Frontend gibt es
  kein `expect` auf Zustand, den der Browser bestimmt.
- Wo er erscheint:
  - Laden einer Seite oder eines Menüs scheitert: Alert auf der Seite (im Menü `MenuError`),
    mit „Erneut laden“.
  - Eine Aktion scheitert, und die Eingaben stehen noch auf der Seite (Speichern, Löschen,
    Sync, Editoren wie Ports, Fasern und Loops): Toast (`util::toast_error`), die Seite bleibt.
  - Im Dialog: Alert im Dialog.
- Der Titel des Toasts ist „<Objekt> konnte nicht gespeichert|angelegt|gelöscht|geladen|
  angestossen|geändert|abgeschlossen|aktiviert werden“.
- „Nicht gefunden“ ist immer `FrontendError::NotFound` mit Art und Id des Objekts („Trasse 7 nicht gefunden“, Alert Danger), nie ein eigener Text.
- Ein Hinweis, der vor dem Speichern gilt (Feld fehlt, Kombination nicht erlaubt), ist ein
  Warning-Alert bei den Feldern und sperrt „Speichern“; er ist kein Fehler.
- Die Konsole meldet nur, was kein Fehler ist (z. B. Schrift nicht geladen).

## 5. Eingabe und Bestätigung von Änderungen

- **Rückmeldung:** Jede Änderung an gespeicherten Daten endet mit einem Toast in der
  Vergangenheit, im selben Muster wie der Fehlertitel: „Schacht gespeichert“, „Kabel angelegt“,
  „Trasse gelöscht“. Das gilt auch für die Editoren von Panels, Ports, Fasern und Loops und
  wenn danach die Seite wechselt.
- **Wohin danach:**
  - Formular für ein Objekt (`Properties`): nach dem Speichern und Anlegen auf die Übersicht
    des Objekts, nach dem Löschen auf die Liste.
  - Editor für eine Arbeitsansicht (Kabel, Panels, Ports, Fasern, Loops): bleibt auf der Seite
    und lädt die gespeicherten Daten neu.
- **Speichern-Knopf:** aktiv nur, wenn es Änderungen gibt und sie gültig sind; ohne Rolle für
  die Änderung gibt es ihn nicht (Seite schreibgeschützt).
- **Löschen:** ein gespeichertes Objekt wird immer erst nach Rückfrage gelöscht
  (`confirm_delete`, `ask_delete`). Was ein Editor nur im Speicher entfernt, fragt nicht, ist
  aber als „nicht gespeichert“ sichtbar, solange „Speichern“ fehlt.
- **Ungespeicherte Änderungen:** Eine Seite mit Änderungen, die noch fehlen, warnt beim
  Verlassen: bei jedem Wechsel in der App (Breadcrumb, Links) mit einem Dialog „Änderungen
  verwerfen?“ (Knöpfe „Verwerfen“ und „Weiter bearbeiten“), beim Schliessen und Neuladen des
  Fensters mit dem Hinweis des Browsers (`beforeunload`); bei „Zurück“ im Browser bleibt die Adresse
  auf der Seite und der Dialog erscheint. `yew-nested-router` (0.9) lässt einen Wechsel nicht
  abfangen: `Link`, `push` und `replace` navigieren sofort, und `popstate` ist nicht zu
  verhindern. Darum wacht `UnsavedGuard` (`components/unsaved.rs`) um den Router selbst: Er fängt
  Klicks auf Links der App und `popstate` in der Capture-Phase ab, bevor der Router sie sieht,
  schiebt bei „Zurück“ die Adresse der Seite wieder auf den Stapel und fragt. Nur „Verwerfen“
  führt weiter. Eine Seite mit Änderungen hält ein `Unsaved` und meldet nach jedem Rendern
  `set(hat_aenderungen)`; ohne Änderungen, schreibgeschützt oder beim Laden meldet sie `false`.
  Was eine Seite selbst öffnet (`util::navigate` nach Speichern oder Löschen), fragt nicht.
  Ein neues Objekt, in dem noch nichts eingegeben ist, hat nichts zu verlieren.
- **Dialoge mit Formular:** „Speichern“ oder „Anlegen“ und „Abbrechen“; Enter sendet ab,
  Abbrechen und Esc schliessen ohne Rückfrage.

## Prüfung

Die Einhaltung wird nur lokal geprüft, nicht in der CI. Vor jedem Review einer Änderung am
Frontend läuft `local/konventionen/run.sh` (baut das Frontend, startet den Mock einmal je Rolle
`ADMIN`, `PLANNER`, `READER`); `SKIP_BUILD=1` nimmt das vorhandene `dist/`, `VERBOSE=1` zeigt die
noch nicht eingeschalteten Prüfungen einzeln. Drei Ebenen, von der strengsten zur weichsten:

| Ebene | Werkzeug | Prüft |
| --- | --- | --- |
| Compiler | `cargo clippy -p cable-editor-frontend --target wasm32-unknown-unknown` (`cable-editor-frontend/clippy.toml`, `[lints.clippy]`) | keine Browser-Dialoge (`alert`, `confirm`, `prompt`), kein `log::error!` (Fehler gehören in die Oberfläche), keine Panics (`unwrap`, `expect`, `panic!`), kein rohes `Link` des Routers |
| Quelltext | `local/konventionen/static.mjs` | Beschriftungen (Abschnitt 1), Fehler als `FrontendError` und Toast-Titel (4), Symbolknöpfe als `IconButton`, „Seite drucken“ nur an einer Stelle, `PlanLink` statt `Link<AppRoute>` (3) |
| Browser | `local/konventionen/pages.mjs` (Playwright gegen den Mock) | Verlassen einer Seite mit Änderungen (Link, „Zurück“, Fenster schliessen), jede Route als Handy und Desktop: keine Konsolenfehler, kein waagerechter Überlauf, genau eine sichtbare `h1`, höchstens ein aktiver Eintrag je Breadcrumb-Menü, „Keine Berechtigung“ ohne die nötige Rolle, jeder Knopf und Link hat einen Namen |

- Eine Prüfung mit „ausstehend“ gehört zu einem Umsetzungsschritt, der noch fehlt; der Commit
  dieses Schritts schaltet sie ein (`step` in den Tabellen der Skripte, Lint in `Cargo.toml`), damit
  die Prüfung danach immer grün ist.
- Eine Ausnahme steht im Code und hat einen Grund: `// konventionen:ignore <regel> <Grund>` in der
  Zeile davor, bei Clippy `#[allow(...)]` mit einem Satz. `run.sh` nennt ihre Zahl.
- `pages.mjs` hat für jede Route der Enums in `pages/router.rs` einen Eintrag (`ROUTES`) und bricht
  ab, wenn einer fehlt oder überzählig ist; eine neue Route gehört also mit ihrem Eintrag in
  denselben Commit.
- `MOCK_FAIL=updateCable,deleteCable` (oder `*`) lässt Mutationen des Mocks wie bei einer
  kaputten Datenbank fehlschlagen, `GET /mock/fail?mutations=…` ändert das im laufenden Mock.
- Jede neue Regel dieser Datei kommt im selben Commit in eine der Ebenen oder in die Prüfliste.

Was sich nicht automatisch prüfen lässt, geht der Reviewer bei jeder Änderung durch:

- [ ] Beschriftungen sind kurze Verben und stimmen mit Abschnitt 1 überein; Sonderfälle sind
      begründet.
- [ ] Jede Aktion, die Daten ändert, endet mit Toast (Erfolg) oder Fehler im Muster von 4 und 5.
- [ ] Jedes Objekt, das die Seite zeigt, ist verlinkt (Abschnitt 3), Ausnahme: Formulare.
- [ ] Das Menü bietet nur eine Ebene an, die Gruppierung der Bereiche stimmt (Abschnitt 2).
- [ ] Texte sind verständlich, deutsch und ohne technische Wörter, die Benutzer nicht kennen.
- [ ] Auf dem Handy (Screenshot) ist alles bedienbar; kein Text ist abgeschnitten.

## Umsetzung

Schrittweise, je ein Commit, jeder vor dem Commit durchgesehen:

1. (erledigt) Fehler als `FrontendError`: Alerts mit `to_string()`, Debug-Ausgaben, eigene
   „nicht gefunden“-Texte, `expect` im Browserzustand, Fehler der Editoren als Toast.
   Ausnahmen mit `#[allow(clippy::expect_used)]`: fehlender Yew-Kontext, `build.rs`. Ein Toast
   zeigt den Titel des `FrontendError` (`util::ToastText`), nicht dessen englischen `Display`.
2. (erledigt) Beschriftungen nach Abschnitt 1 (auch „Etikett drucken“ und „Seite drucken“),
   `aria_label` und `title` der Symbolknöpfe (`components/icon_button.rs` `IconButton`, denn
   patternfly-yew kennt kein `title`; `PopupMenu` setzt es aus seinem `aria_label`),
   Bestätigungsdialog (`confirm_delete(scope, Art, Name, …)`, Titel „<Art> löschen?“).
   „Seite drucken“ ist `components/print_page.rs`.
3. (erledigt) Rückmeldung nach Abschnitt 5: Toasts in den Editoren, beim Löschen und beim Anlegen
   einer Planung oder eines Kabels; „Speichern“ und „Anlegen“ nur aktiv bei Änderungen. Ein
   Dialog im Backdrop kennt den Toaster nicht (der `BackdropViewer` liegt ausserhalb des
   `ToastViewer`): er meldet Erfolg und Fehler der Seite, die ihn geöffnet hat, und die zeigt den
   Toast (`NewPlanDialog`). Beim Löschen kommt der Toast vor dem Wechsel auf die
   Liste.
4. (erledigt) Links für Schachttyp, Eigentümer und Plan; `PlanLink` statt `Link<AppRoute>`.
   `components/links.rs` hat `SchachtTypLink` (Seite des Typs) und `OwnerLink` (Liste der
   Eigentümer), `plan_link.rs` neben `PlanLink` (bleibt im Plan des Pfads) `PlanNameLink` für
   einen anderen Plan (auf dessen Schächte, wie die Liste der Planungen). Clippy verbietet
   `Link` ausserhalb von `plan_link.rs` (`disallowed-types`).
5. (erledigt) Bereichsmenü gruppiert (`MenuDropdown` mit `groups`); „Planung“ statt „Plan“ in der
   Oberfläche (die Regel `plan-statt-planung` prüft die Texte); das Kabel auf einer Seite für
   Ansehen, Anlegen und Bearbeiten (`PlanView::NewCable { id }`, `EditCable` mit `IdOrNew`, kein
   Dialog). `createCable` nimmt Name, Fasern und Kabelweg auf einmal und verweigert einen leeren
   Weg, damit es kein Kabel ohne Segment gibt; danach öffnet die Kabelseite.
6. (erledigt) Warnung bei ungespeicherten Änderungen (`components/unsaved.rs`): `UnsavedGuard`
   um den Router, `Unsaved` in den Formularen der Schächte, Trassen, Schachttypen, Kabel, in
   Planung, Panel-Editor, Fasern auflegen und Loops. `pages.mjs` prüft Link, „Zurück“ und
   das Schliessen des Fensters (`LEAVE_FLOWS`) und dass nach dem Speichern nicht mehr gefragt wird.

## Entschieden

- **Kabel anlegen und bearbeiten:** eine Seite mit `IdOrNew` als Schlüssel, kein Dialog; das
  Anlegen unterscheidet sich nur in Beschriftung und Vorgaben.
- **Kabelseite:** Übersicht und Editor bleiben eine Seite; Leser sehen die Einstellungen,
  Berechtigte ändern sie dort. Bei Schacht und Trasse bleibt die Übersicht mit dem Knopf
  „Bearbeiten“, weil sie viel enthält, was nicht zum Formular gehört (Panels, Etiketten, Kabel).
- **Planung:** so heisst der Plan in der Oberfläche.
- **Warnung beim Verlassen:** die App fragt selbst nach (siehe 5) und fängt Klicks und „Zurück“
  global ab, weil der Router den Wechsel nicht abfangen lässt; dadurch muss nicht jeder Link
  etwas davon wissen.
