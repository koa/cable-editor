# Konzept: Netbox automatisch synchronisieren

Stand: 28.09.2026 – umgesetzt.

## Ziel

Netbox zeigt immer die Circuits des in Netbox aktiven Plans. Jede Änderung, die sich auf diese
Circuits auswirkt, stösst einen Sync an. Findet der Sync Probleme (`SyncIssue`), schreibt er
nichts nach Netbox, sondern speichert die Probleme. Admins sehen sie auf einer eigenen Seite,
alle anderen sehen den Hinweis, dass Netbox nicht synchron ist.

Synchronisiert werden weiterhin nur die Circuits (anlegen, Terminations und Kabel zum Rear Port,
Beschreibung und Länge aktualisieren, eigene `FIBER-…`-Circuits löschen), genau wie früher der
manuelle Sync (`mutation/sync.rs`, `sync_plan_to_netbox`). Devices, Ports und alles andere in
Netbox bleiben unberührt.

## Der aktive Plan

- Die Spalte `plan.netbox_active` (Migration `netbox-sync`, höchstens ein Plan per partiellem
  Unique-Index, anfangs der Ist-Zustand).
- Ein Admin macht einen Plan aktiv (`setNetboxActivePlan`). Das ersetzt den Button „Sync Netbox“
  der Planungsseite (siehe UI). Danach läuft der Sync automatisch.
- Wird der aktive Plan implementiert, wird der Ist-Zustand aktiv (`implement.rs`). Der Stand in
  Netbox ändert sich dadurch nicht, weil der Ist-Zustand danach genau diesem Plan entspricht. Ein
  anderer Weg, Pläne zu löschen, existiert nicht.
- Ein geplanter Plan besteht aus dem Ist-Zustand und seinen Änderungen. Ist er aktiv, wirken sich
  deshalb auch Änderungen am Ist-Zustand auf Netbox aus.

## Was einen Sync auslöst

Die Datenbank merkt sich, dass ein Sync nötig ist. Triggers fügen in derselben Transaktion wie
die Änderung eine Zeile in `netbox_sync_anstoss` ein, eine pro Transaktion (`txid`). So geht
kein Anstoss verloren, auch nicht bei einem Neustart oder einem Import mit psql. Ein
zurückgerollter Request stösst auch keinen Sync an. Zeilen statt eines Flags in einer
gemeinsamen Zeile: So warten die Transaktionen nicht aufeinander, und ein Lauf kann genau die
Anstösse löschen, die er gesehen hat. Die Triggers (Migration `netbox-auto-sync`) hängen an
allem, was der Sync liest, und laufen pro Statement, ausser wo sie die Zeile prüfen müssen:

| Tabelle | warum |
|---|---|
| `port_usage`, pro Zeile: nur die des Ist-Zustands und des aktiven Plans | Faserwege (`trace_fiber_path`) |
| `panel_port` | Connector-Ports, `netbox_port_id`, Label (Beschreibung des Circuits) |
| `panel` | Panels kommen hinzu oder fallen weg (mit ihren Ports) |
| `kabel`, `kabel_trasse` | Faserwege, Länge (`distance`) |
| `trasse` (Geometrie, Schächte), `schacht` (Geometrie) | Länge über `trassen_mit_endpunkten` |
| `plan`, pro Zeile: nur wenn sich `netbox_active` ändert (Umbenennen schreibt alle Spalten) | ein anderer Plan wird aktiv |

Ein Trigger, der ohne Grund feuert (z. B. bei einem neuen Kabel ohne Fasern), schadet nicht.
Der Sync findet dann nichts zu tun.

## Ablauf

- Das Binary startet einen Worker (`netbox/auto_sync.rs`). Er wacht nach jedem committeten
  Request auf (`tokio::sync::Notify` aus `main.rs`), dazu jede Minute.
- Ein Lauf ist fällig, wenn es Anstösse gibt oder der letzte Lauf länger als das Intervall
  zurückliegt (Config `netbox.sync_interval_h`, Standard 4 Stunden, im Helm-Chart
  `netboxSyncIntervalHours`). So gleicht ein Lauf auch Änderungen aus, die jemand in Netbox von
  Hand gemacht hat. Das Intervall zählt ab dem letzten Lauf und hängt an keiner Tageszeit,
  Zeitzonen spielen also keine Rolle.
- Ein Lauf ist eine Transaktion auf einer eigenen Verbindung:
  - `pg_try_advisory_xact_lock`, damit mehrere Replicas nicht gleichzeitig arbeiten (mit der
    Transaktion wieder frei).
  - Ist eine Wartezeit nach einem Fehler offen (`naechster_versuch`) oder nichts fällig, endet
    er hier.
  - Er liest die Anstösse, synchronisiert den aktiven Plan (ohne aktiven Plan den Ist-Zustand),
    speichert das Ergebnis und löscht genau die gelesenen Anstösse.
- Kommt während des Laufs eine Änderung, bleibt ihr Anstoss stehen. Der nächste Lauf nimmt sie
  mit.
- Bricht ein Lauf ab (Neustart, Absturz), rollt die Datenbank ihn zurück: Die Anstösse bleiben,
  der nächste Lauf holt ihn nach.
- **Issues**: Nach Netbox wird nichts geschrieben. Die Issues werden gespeichert, das Ergebnis
  ist `issues`. Ein neuer Lauf kommt mit der nächsten Änderung, nach dem Intervall oder mit
  `syncNetbox`.
- **Technischer Fehler** (Netbox nicht erreichbar, Netbox weist einen Schritt ab): Der Fehler
  wird gespeichert (mit seiner Herkunft und Id, siehe `fehlermeldungen.md`), das Ergebnis ist
  `fehler`. Der Lauf wird zurückgerollt, seine Anstösse bleiben also. Den Fehler speichert eine
  eigene kurze Transaktion danach, denn nach einem Datenbankfehler ist die des Laufs
  abgebrochen; committet wird so nie mehr als eine. Neue Versuche folgen nach 1, 2, 5 und dann alle 10
  Minuten (`naechster_versuch`, in der Datenbank, damit alle Replicas ihn kennen); Änderungen
  verkürzen die Wartezeit nicht, `syncNetbox` schon. Ein Abbruch mitten in den Netbox-Schritten
  hinterlässt Netbox teilweise synchronisiert. Der nächste erfolgreiche Lauf gleicht das aus,
  weil der Sync den Soll-Zustand jedes Mal ganz vergleicht.
- **Erfolg**: Die gespeicherten Issues werden gelöscht, das Ergebnis ist `ok`, die Fehlversuche
  zählen wieder von vorn.
- Jeder Lauf steht im Server-Log (`Netbox sync: Synced`, `Issues(3)`, `Failed`), ein Fehler mit
  seiner Id.

## Gespeicherter Zustand

- `netbox_sync_anstoss` (id, `txid`, `erstellt`): die Änderungen, die noch kein Lauf
  synchronisiert hat.
- `netbox_sync` (eine Zeile, nur der Worker schreibt sie, `syncNetbox` hebt die Wartezeit auf):
  - `letzter_lauf`
  - `ergebnis` (`ok` | `issues` | `fehler`)
  - `fehler` (jsonb: der GraphQL-Fehler, `message` und `extensions` mit dem `userError` wie
    `NetboxFailed` oder der Herkunft)
  - `fehlversuche`, `naechster_versuch` (die Wartezeit nach Fehlern)
- `netbox_sync_issue` (id, `daten` jsonb): Die Issues des letzten Laufs, bei jedem Lauf ganz
  ersetzt.
  - Gespeichert werden die Ids der Ports und Panels und eine Momentaufnahme der Netbox-Rear-Ports
    (`NetboxPortRef`: Id, Name, Device, Standort), jeweils mit serde.
  - Beim Lesen baut GraphQL daraus die bestehende Union `SyncIssue`: Ports über den Loader, Rear
    Ports aus der Momentaufnahme.
  - Das Frontend behält so seine Texte (`error/messages.rs`).
  - Ein Issue, dessen Port oder Panel inzwischen gelöscht ist, fällt weg. Die Löschung hat
    ohnehin einen neuen Lauf angestossen.
  - Wenn der letzte Sync erfolgreich war, ist die Tabelle leer. Nach einem Fehler auch: welche
    Issues der Plan hat, ist dann nicht bekannt.

## GraphQL

- Query `netboxSync` (`graphql/authenticated/netbox_sync.rs`):
  - Für alle: `state` (`SYNCHRON`, `AUSSTEHEND`, `NICHT_SYNCHRON`, `FEHLER`; bei Issues oder
    Fehler bleibt der Stand des letzten Laufs, bis ein neuer fertig ist), `pending` (es gibt
    Anstösse), `lastRun`, `retryAt`, `activePlan`. Dass ein Lauf gerade läuft, ist ausserhalb
    seiner Transaktion nicht zu sehen.
  - Nur für Admins (Guard pro Feld): `issues: [SyncIssue!]!` und `error` (`message`,
    `extensions` als JSON).
- `Plan.netboxActive`.
- Mutation `setNetboxActivePlan(planId)` (Admin): Setzt den Plan aktiv und gibt ihn zurück (ein
  fehlender: `UserError::NotFound`). Der Trigger stösst den Sync an. Ersetzt `syncPlanToNetbox`.
- Mutation `syncNetbox` (Admin): Fügt einen Anstoss ein und hebt die Wartezeit nach einem
  Fehler auf, z. B. nachdem jemand in Netbox etwas von Hand korrigiert hat.
- Der Mock liefert denselben Zustand, umschaltbar für Screenshots (`MOCK_NETBOX=OK`, `ISSUES`,
  `FEHLER`, sonst noch kein Lauf), und für `local/realdb` eine Netbox ohne Circuits
  (`/netbox/graphql/`), deren Abfragen `check.mjs` zählt.

## UI

- **Hinweis in der Breadcrumb-Leiste** neben dem `UserMenu` (`components/netbox.rs`
  `NetboxHint`), nur wenn Netbox nicht synchron ist:
  - Ein Menü wie das des Benutzers: „Netbox nicht synchron“ (Issues) bzw. „Netbox-Sync
    fehlgeschlagen“ (Fehler), auf dem Handy nur das Symbol; geöffnet erklärt es den Stand, für
    Admins mit dem Weg zur Netbox-Seite, für alle anderen mit dem Hinweis, dass Admins die Gründe
    sehen.
  - `AUSSTEHEND` zeigt nichts an, das ist der Normalfall nach jeder Änderung.
  - Geladen wird er bei jeder Navigation und jede Minute (`gloo-timers`); konnte er nicht laden,
    zeigt er „Netbox-Status unbekannt“ mit dem Fehler.
- **Seite „Netbox“** (Admins, `PlanView::Netbox`, `pages/netbox.rs`, unter dem Pfad des Plans
  wie die anderen Seiten, im Menü wie „Leitungskataster“):
  - Stand, aktiver Plan mit Link, letzter Lauf, nach einem Fehler der nächste Versuch.
  - Ein Fehler mit Details und Herkunft.
  - Die Issues mit den Texten aus `error/messages.rs` (`components/netbox.rs` `view_issues`).
  - Button „Jetzt synchronisieren“; solange ein Lauf aussteht, fragt die Seite alle 5 s neu.
- **Aktiver Plan statt „Sync Netbox“**:
  - Die Planungsseite zeigt beim aktiven Plan ein Label „In Netbox aktiv“.
  - Bei allen anderen Plänen gibt es für Admins „In Netbox aktivieren“, mit Bestätigung im
    Backdrop: „Netbox zeigt danach die Circuits dieses Plans“.
  - Die Planübersicht markiert den aktiven Plan ebenfalls.

## Entscheide

- Änderungen, die jemand in Netbox von Hand macht, gleicht der Sync im festen Intervall aus
  (alle paar Stunden, siehe Ablauf). Admins können ihn auch sofort anstossen.
- Nur Admins dürfen einen Plan in Netbox aktivieren, wie früher den manuellen Sync.

## Prüfung

- `local/realdb/check.mjs` (gegen die Netbox des Mocks): der Lauf nach dem Aufbau, keiner nach
  einer Änderung eines nicht aktiven Plans oder dem Umbenennen, einer nach dem Aktivieren, nach
  einer Änderung des aktiven Plans und eines Kabels, der Ist-Zustand wird beim Implementieren des
  aktiven Plans aktiv, `syncNetbox`, `setNetboxActivePlan` eines fehlenden Plans.
- Von Hand: ein Lauf, der an der Datenbank scheitert, wartet 1, dann 2 Minuten, sein Anstoss
  bleibt; `syncNetbox` überspringt die Wartezeit, danach ist der Stand wieder synchron und der
  Anstoss gelöscht.
- Unit-Test: die Issues überstehen das Speichern (`mutation/sync.rs`).
