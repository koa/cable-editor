# Konzept: Netbox automatisch synchronisieren

Stand: 28.09.2026 – Konzept, nicht umgesetzt.

## Ziel

Netbox zeigt immer die Circuits des in Netbox aktiven Plans. Jede Änderung, die sich auf diese
Circuits auswirkt, stösst einen Sync an. Findet der Sync Probleme (`SyncIssue`), schreibt er
nichts nach Netbox, sondern speichert die Probleme. Admins sehen sie auf einer eigenen Seite,
alle anderen sehen den Hinweis, dass Netbox nicht synchron ist.

Synchronisiert werden weiterhin nur die Circuits (anlegen, Terminations und Kabel zum Rear Port,
Beschreibung und Länge aktualisieren, eigene `FIBER-…`-Circuits löschen), genau wie heute beim
manuellen Sync (`mutation/sync.rs`). Devices, Ports und alles andere in Netbox bleiben
unberührt.

## Der aktive Plan

- Die Spalte `plan.netbox_active` gibt es schon (Migration `netbox-sync`, höchstens ein Plan per
  partiellem Unique-Index, anfangs der Ist-Zustand). In `db/schema.rs` fehlt sie noch.
- Ein Admin macht einen Plan aktiv. Das ersetzt den Button „Sync Netbox“ der Planungsseite (siehe
  UI). Danach läuft der Sync automatisch.
- Wird der aktive Plan implementiert, wird der Ist-Zustand aktiv (`implement.rs`). Der Stand in
  Netbox ändert sich dadurch nicht, weil der Ist-Zustand danach genau diesem Plan entspricht. Ein
  anderer Weg, Pläne zu löschen, existiert nicht.
- Ein geplanter Plan besteht aus dem Ist-Zustand und seinen Änderungen. Ist er aktiv, wirken sich
  deshalb auch Änderungen am Ist-Zustand auf Netbox aus.

## Was einen Sync auslöst

Die Datenbank merkt sich, dass ein Sync nötig ist. Triggers setzen in derselben Transaktion wie
die Änderung `netbox_sync.pending = true`. So geht kein Anstoss verloren, auch nicht bei einem
Neustart oder einem Import mit psql. Ein zurückgerollter Request stösst auch keinen Sync an. Die
Triggers laufen pro Statement, nicht pro Zeile, und hängen an allem, was der Sync liest:

| Tabelle | warum |
|---|---|
| `port_usage` (Zeilen des Ist-Zustands und des aktiven Plans) | Faserwege (`trace_fiber_path`) |
| `panel_port` | Connector-Ports, `netbox_port_id`, Label (Beschreibung des Circuits) |
| `panel` | Panels kommen hinzu oder fallen weg (mit ihren Ports) |
| `kabel`, `kabel_trasse` | Faserwege, Länge (`distance`) |
| `trasse` (Geometrie, Schächte), `schacht` (Geometrie) | Länge über `trassen_mit_endpunkten` |
| `plan` (`netbox_active`) | ein anderer Plan wird aktiv |

Ein Trigger, der ohne Grund feuert (z. B. bei einem neuen Kabel ohne Fasern), schadet nicht.
Der Sync findet dann nichts zu tun.

## Ablauf

- Das Binary startet einen Worker (tokio-Task). Er wacht nach jedem committeten Request auf
  (`tokio::sync::Notify` aus `main.rs`), dazu jede Minute.
- Liegt der letzte Lauf länger als das Intervall zurück (Config `netbox.sync_interval_h`,
  Standard 4 Stunden), setzt der Worker `pending` selbst. So gleicht ein Lauf auch Änderungen
  aus, die jemand in Netbox von Hand gemacht hat. Das Intervall zählt ab dem letzten Lauf und
  hängt an keiner Tageszeit, Zeitzonen spielen also keine Rolle.
- Ist `pending` gesetzt, holt er sich das Recht zu synchronisieren mit
  `pg_try_advisory_lock`, damit mehrere Replicas nicht gleichzeitig arbeiten. Er setzt
  `pending = false`, committet und ruft danach den bestehenden Sync für den aktiven Plan auf.
  `sync_plan_to_netbox` bekommt dafür eine eigene Verbindung statt der des Requests.
- Kommt während des Syncs eine Änderung, setzt der Trigger `pending` erneut. Der nächste Lauf
  nimmt sie dann mit.
- **Issues**: Nach Netbox wird nichts geschrieben. Die Issues werden gespeichert, das Ergebnis
  ist `issues`. Ein neuer Lauf kommt erst mit der nächsten Änderung.
- **Technischer Fehler** (Netbox nicht erreichbar, Netbox weist einen Schritt ab): Der Fehler
  wird gespeichert, das Ergebnis ist `fehler`, und `pending` wird wieder gesetzt. Neue Versuche
  folgen nach 1, 2, 5 und dann alle 10 Minuten. Ein Abbruch mitten in den Netbox-Schritten
  hinterlässt Netbox teilweise synchronisiert. Der nächste erfolgreiche Lauf gleicht das aus,
  weil der Sync den Soll-Zustand jedes Mal ganz vergleicht.
- **Erfolg**: Die gespeicherten Issues werden gelöscht, das Ergebnis ist `ok`.

## Gespeicherter Zustand

- `netbox_sync` (eine Zeile):
  - `pending`
  - `laeuft_seit` (gesetzt während eines Laufs)
  - `letzter_lauf`
  - `ergebnis` (`ok` | `issues` | `fehler`)
  - `fehler` (jsonb: der `UserError` wie `NetboxFailed`, sonst die technische Meldung mit ihren
    Details)
- `netbox_sync_issue` (id, `daten` jsonb): Die Issues des letzten Laufs, bei jedem Lauf ganz
  ersetzt.
  - Gespeichert werden die Ids der Ports und eine Momentaufnahme der Netbox-Rear-Ports (Id, Name,
    Device), jeweils mit serde.
  - Beim Lesen baut GraphQL daraus die bestehende Union `SyncIssue`: Ports über den Loader, Rear
    Ports aus der Momentaufnahme.
  - Das Frontend behält so seine Texte (`error/messages.rs`).
  - Ein inzwischen gelöschter Port fällt weg. Seine Löschung hat ohnehin einen neuen Lauf
    angestossen.

## GraphQL

- Query `netboxSync`:
  - Für alle: `state` (`SYNCHRON`, `AUSSTEHEND`, `NICHT_SYNCHRON`, `FEHLER`), `activePlan`,
    `lastRun`.
  - Nur für Admins (Guard pro Feld): `issues: [SyncIssue!]!` und `error`.
- Mutation `setNetboxActivePlan(planId)` (Admin): Setzt den Plan aktiv. Der Trigger stösst den
  Sync an. Ersetzt `syncPlanToNetbox`.
- Mutation `syncNetbox` (Admin): Setzt `pending`, z. B. nachdem jemand in Netbox etwas von Hand
  korrigiert hat.
- Der Mock liefert denselben Zustand, umschaltbar für Screenshots.

## UI

- **Hinweis in der Breadcrumb-Leiste** neben dem `UserMenu`, nur wenn Netbox nicht synchron
  ist:
  - Anzeige: „Netbox nicht synchron“ (Issues) bzw. „Netbox-Sync fehlgeschlagen“ (Fehler).
  - Für Admins ein Link zur Netbox-Seite, für alle anderen nur der Hinweis mit Tooltip.
  - `AUSSTEHEND` zeigt nichts an, das ist der Normalfall nach jeder Änderung.
  - Geladen wird er bei jeder Navigation und jede Minute (`gloo-timers`).
- **Seite „Netbox“** (Admins, `PlanView::Netbox`, unter dem Pfad des Plans wie die anderen
  Seiten, im Menü wie „Leitungskataster“):
  - Aktiver Plan mit Link, letzter Lauf und Ergebnis.
  - Die Issues mit den Texten und der Darstellung aus `NetboxSyncModal`, das dafür zur
    Komponente wird.
  - Ein Fehler mit Details.
  - Button „Jetzt synchronisieren“.
- **Aktiver Plan statt „Sync Netbox“**:
  - Die Planungsseite zeigt beim aktiven Plan ein Label „In Netbox aktiv“.
  - Bei allen anderen Plänen gibt es für Admins „In Netbox aktivieren“. Das ist ein Menüeintrag
    statt eines Buttons, mit Bestätigung im Backdrop: „Netbox zeigt danach die Circuits dieses
    Plans“.
  - Die Planübersicht markiert den aktiven Plan ebenfalls.

## Entscheide

- Änderungen, die jemand in Netbox von Hand macht, gleicht der Sync im festen Intervall aus
  (alle paar Stunden, siehe Ablauf). Admins können ihn auch sofort anstossen.
- Nur Admins dürfen einen Plan in Netbox aktivieren, wie heute den Sync.

## Umsetzung (Commits)

1. `netbox_active` in `db/schema.rs`, der Ist-Zustand wird beim Implementieren aktiv, und
   `setNetboxActivePlan` ersetzt `syncPlanToNetbox`. Im Frontend gibt es dafür „In Netbox
   aktivieren“ und das Label.
2. Die Tabellen `netbox_sync` und `netbox_sync_issue`, die Triggers, der Worker im Binary mit
   dem Intervall in der Config (auch im Helm-Chart) und
   `sync_plan_to_netbox` mit eigener Verbindung. Dazu `check.mjs` für die Triggers (ohne Netbox:
   Ergebnis `fehler`).
3. Die Query `netboxSync` und die Mutation `syncNetbox`, dazu Hinweis und Netbox-Seite im
   Frontend und der Mock.
