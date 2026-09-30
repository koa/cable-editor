# Konzept: Einheitliche Bedienung im Frontend

Stand: 30.09.2026 – offene Fragen entschieden, noch nicht umgesetzt.

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
    verbunden“) und der Knopf in Tabellenzeilen, wo aus dem Zusammenhang klar ist, welches
    Etikett gemeint ist (Symbol Etikett, `aria_label` „Etikett drucken“).
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
  angestossen werden“.
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
  Fensters mit dem Hinweis des Browsers (`beforeunload`). `yew-nested-router` (0.9) lässt einen
  Wechsel nicht abfangen: `Link`, `push` und `replace` navigieren sofort, und `popstate` (Zurück
  im Browser) ist nicht zu verhindern. Darum fragt die App selbst: Eine Seite mit ungespeicherten
  Änderungen meldet sich in einem Kontext an, und alle Wege aus der App (`PlanLink`, Menüeinträge,
  `util::navigate`) fragen dort zuerst nach; bei „Zurück“ im Browser schiebt die App die Adresse
  wieder auf die Seite und fragt dann. Deshalb darf keine Seite ein rohes `Link<AppRoute>` bauen.
- **Dialoge mit Formular:** „Speichern“ oder „Anlegen“ und „Abbrechen“; Enter sendet ab,
  Abbrechen und Esc schliessen ohne Rückfrage.

## Umsetzung

Schrittweise, je ein Commit, jeder vor dem Commit durchgesehen:

1. Fehler als `FrontendError`: Alerts mit `to_string()`, Debug-Ausgaben, eigene
   „nicht gefunden“-Texte, `expect` im Browserzustand, Fehler der Editoren als Toast.
2. Beschriftungen nach Abschnitt 1 (auch „Etikett drucken“ und „Seite drucken“), `aria_label` und
   `title` der Symbolknöpfe, Bestätigungsdialog.
3. Rückmeldung nach Abschnitt 5: Toasts in den Editoren, beim Löschen und beim Anlegen einer
   Planung; „Speichern“ nur aktiv bei Änderungen.
4. Links für Schachttyp, Eigentümer und Plan; `PlanLink` statt `Link<AppRoute>`.
5. Bereichsmenü gruppieren; „Planung“ statt „Plan“ in der Oberfläche; das Kabel auf eine Seite für
   Ansehen, Anlegen und Bearbeiten (`NewCable` mit `IdOrNew`, kein Dialog).
6. Warnung bei ungespeicherten Änderungen.

## Entschieden

- **Kabel anlegen und bearbeiten:** eine Seite mit `IdOrNew` als Schlüssel, kein Dialog; das
  Anlegen unterscheidet sich nur in Beschriftung und Vorgaben.
- **Kabelseite:** Übersicht und Editor bleiben eine Seite; Leser sehen die Einstellungen,
  Berechtigte ändern sie dort. Bei Schacht und Trasse bleibt die Übersicht mit dem Knopf
  „Bearbeiten“, weil sie viel enthält, was nicht zum Formular gehört (Panels, Etiketten, Kabel).
- **Planung:** so heisst der Plan in der Oberfläche.
- **Warnung beim Verlassen:** die App fragt selbst nach (siehe 5), weil der Router den Wechsel
  nicht abfangen lässt.
