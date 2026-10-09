# Konzept: Port-Belegungen prüfen

Stand: 09.10.2026 – umgesetzt.

## Ziel

Eine Port-Belegung (`port_usage`) hängt eine Faser eines Kabels an eine Seite eines Ports. Sie
passt nur, solange das Kabel im Schacht des Panels endet und die Faser hat. Ändert sich der
Kabelweg oder die Faserzahl, kann eine Belegung ungültig werden. Bis jetzt hat das niemand
geprüft. Der Loop-Editor zeigte dann zum Beispiel keine Kabel mehr an, obwohl seine Ports belegt
waren.

Änderungen, die Belegungen ungültig machen, werden abgelehnt. Was schon ungültig ist, sehen
Admins und können es reparieren.

## Was nicht passt

Die eine Definition ist der View `port_usage_issue` (Migration `port-usage-issue`). Er enthält
eine Zeile pro Problem einer Belegung, in allen Planungen, auch im Ist-Zustand:

| Problem | wann |
|---|---|
| `CableNotEnding` | Das Kabel endet nicht im Schacht des Panels. |
| `FiberOutOfRange` | Das Kabel hat dieses Bündel oder diese Faser nicht. |
| `FiberTwice` | Dasselbe Faserende liegt in der Planung (`effective_port_usage`) an einer weiteren Port-Seite im selben Schacht. |

Wo ein Kabel endet, sagt der View `kabel_ende`: in den Schächten, die nur eine seiner Trassen
berührt. Ein Schacht, durch den das Kabel läuft, wird von zwei Trassen berührt. `Schacht.cables`
nutzt denselben View, so dass die Editoren genau die Kabel anbieten, die die Prüfung erlaubt.

Die Trigger der ersten Panel-Migration (`check_faser_limits`, `check_kabel_update_limits`)
ersetzt der View. Sie prüften jede Zeile für sich, kannten den Kabelweg nicht und meldeten
deutsche Texte als unerwartete Fehler.

## Ablehnen

`IssueCheck` (`db/entity/port_usage_issue.rs`) liest vor einer Änderung die Probleme in ihrem
Bereich und danach nochmals, im selben Request und damit in derselben Transaktion. Kommen neue
hinzu, wird der Request mit `UserError::PortUsagesBroken` abgelehnt und zurückgerollt. Probleme,
die schon bestanden, blockieren nichts: Sonst könnte man sie nicht beheben, zum Beispiel indem
man den Kabelweg zurückstellt.

| Mutation | Bereich |
|---|---|
| `setPortUsage` | die geänderten Ports der Planung |
| `updateCable` mit Weg oder Fasern | alle Belegungen des Kabels, in allen Planungen |
| `implementPlan` | die Ports der Planung im Ist-Zustand (die Stammdaten können sich seit dem Planen geändert haben) |

Der Fehler enthält nur Ids: Planung, Port, Kabel, Bündel und Faser sowie das Problem. Die Faser
gehört dazu, weil eine abgelehnte Belegung nie gespeichert wurde. Der Toast nennt die Anzahl und
listet darunter höchstens acht Belegungen auf („Ist-Zustand · Berg: Rack > Kassette 1 : 5 · K1
1-5 · das Kabel endet nicht im Schacht“). Die Namen lädt `BrokenPortUsageList`
(`components/port_usage_issues.rs`) beim Anzeigen nach, über die Query `ports` sowie die Namen
der Kabel und Planungen.

## Erkennen und löschen

Die Seite „Datenprüfung“ (`PlanView::Datenpruefung`, nur für Admins, im Bereichsmenü unter
Stammdaten) zeigt alle Belegungen aus dem View, eine Zeile pro Belegung mit allen ihren Problemen:
Planung, Port (verlinkt auf das Panel in dieser Planung), Seite, Kabel und Faser, Problem. Die
Query dafür ist `portUsageIssues`.

„Löschen“ gibt es pro Zeile und als „Alle löschen“ (`removeBrokenPortUsages`). Im Ist-Zustand
wird die Zeile gelöscht, in einer Planung wird die Faser entfernt (eine Zeile ohne Kabel). Das
ist neben dem Umsetzen die einzige Änderung am Ist-Zustand, weil dessen Daten falsch sind.
Gelöscht wird nur, was der View dann noch meldet. Liegt eine Faser an zwei Ports, bleibt sie
deshalb am zweiten, sobald der erste gelöscht ist. Wer eine Faser anders auflegen will, tut das
im Panel.

Solange es solche Belegungen gibt, zeigt `BrokenPortUsageHint` in der Breadcrumb-Leiste, wie
viele es sind (`brokenPortUsageCount`, für alle sichtbar). Admins finden dort den Weg zur Seite.
Der Hinweis fragt bei jeder Navigation und jede Minute neu nach, nach dem Löschen sofort. Der
Mock hat eine solche Belegung mit `MOCK_ISSUES=1`.

## In den Panels lösen

Der Loop-Editor und „Fasern auflegen“ bieten die Kabel an, die im Schacht enden
(`Schacht.cables`), dazu die Kabel, die in der Planung an seinen Ports liegen, aber nicht im
Schacht enden (`Schacht.strayCables(planId)`). Bei einem solchen Kabel fehlt der Weg
(`CableEnd.path` ist leer), ebenso das andere Ende seiner Fasern. Die Editoren markieren es mit
„endet nicht in diesem Schacht“. Der Loop-Editor nimmt Kabel A von der Front der Loop-Ports und
Kabel B von deren Rückseite. So lassen sich die Loops eines solchen Kabels auftrennen und seine
Fasern entfernen. Neue Fasern bietet „Fasern auflegen“ nur von Kabeln an, die im Schacht enden.
Die Übersicht eines Panels markiert Belegungen, deren Kabel nicht im Schacht endet oder die
Faser nicht hat. Eine doppelt belegte Faser zeigt nur die Seite „Datenprüfung“.
