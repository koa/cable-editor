# Konzept: Port-Belegungen prüfen

Stand: 09.10.2026 – Prüfung bei Änderungen umgesetzt; Seite „Datenprüfung“, Hinweis und
Reparatur noch offen.

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

## Offen

- Seite „Datenprüfung“ (Admins, Stammdaten): alle Zeilen des Views mit Planung, Port, Kabel und
  Faser, Problem; „Entfernen“ pro Zeile und für alle. Entfernt werden nur Zeilen, die der View
  noch meldet, auch im Ist-Zustand. Das ist die eine direkte Änderung des Ist-Zustands, denn
  dessen Daten stimmen nicht.
- Hinweis in der Breadcrumb-Leiste wie bei Netbox, solange es Probleme gibt; für Admins mit dem
  Weg zur Seite.
- Loop-Editor und „Fasern auflegen“ bieten auch die Kabel an, die an ihren Ports liegen, aber
  nicht mehr im Schacht enden. Sie sind markiert, damit sich die Belegung lösen lässt. Die
  Übersicht eines Panels markiert die betroffenen Ports.
