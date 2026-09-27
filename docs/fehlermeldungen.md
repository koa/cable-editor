# Konzept: Fehlermeldungen

Stand: 27.09.2026 – umgesetzt.

## Grundsatz

Texte für Benutzer entstehen nur im Frontend. Das Backend liefert, was eine Meldung braucht, in
strukturierter Form; alle konstanten Texte und Muster (z. B. die Form einer UID) liegen im
Frontend an einem Ort.

## Zwei Arten von Problemen

- **Weigerung**: eine Anfrage wird abgelehnt (Eingabe ungültig, Name schon vergeben, noch
  referenziert, nicht gefunden, keine Berechtigung). Es gibt einen Grund, das UI zeigt eine
  Meldung, die Transaktion der Anfrage wird zurückgerollt. → **Fehlerstruktur** `UserError`.
- **Bericht**: Probleme sind das Ergebnis einer Prüfung, meist mehrere, mit Verweisen auf
  Objekte, oder das UI reagiert anders als mit einer Meldung. → **expliziter Typ im Schema**,
  z. B. `SyncIssue` von `syncPlanToNetbox` (Ports als Objekte, alle Probleme auf einmal, vor dem
  Schreiben) oder `DuctLineCheck.needsConfirmation` (führt zur Checkbox „Trotzdem verwenden“).

Die Abwägung: explizite Antworten im Schema sind als Vertrag sichtbarer, brauchen aber pro
Mutation eigene Union-Typen, und eine Anfrage mit Fehler als Antwort wird committet (siehe
Transaktion pro Anfrage in `binary/src/main.rs`); Guards, Loader und technische Fehler blieben
ohnehin GraphQL-Fehler. Für Weigerungen ist die Fehlerstruktur deshalb einfacher und sicherer.

## Fehlerstruktur

- Crate `cable-editor-common` (nur serde, läuft auch im wasm-Frontend): Enum `UserError`, ein
  Variant pro Grund, mit den Daten der Meldung (Namen, Ids, Anzahlen, Grenzen), ohne Text;
  dazu die gemeinsamen Grenzen (`limits`) und Prüfungen (UID).
- Backend: Prüfungen geben `UserError` zurück; `From<UserError> for async_graphql::Error`
  (Feature `async-graphql` des Crates) legt ihn serialisiert in `extensions.userError` ab
  (`{"code": "NameTaken", "kind": "Owner", "name": …}`), `message` ist nur der Code (Hilfe
  beim Debuggen).
- Frontend: `graphql::query`/`mutate` lesen die Extensions typisiert
  (`GraphQlResponse<T, ErrorExtensions>`); ein `UserError` wird `FrontendError::User`, der Text
  kommt aus `error/messages.rs` (vollständiges `match`: ein neuer Variant ohne Text kompiliert
  nicht). Seiten setzen wie bisher nur den Titel („Trasse konnte nicht gespeichert werden“).
- Technische Fehler (Datenbank, Netbox-HTTP, der Start der Transaktion) bleiben unstrukturiert;
  das Frontend zeigt sie mit eigenem Titel („Unerwarteter Fehler vom Server“) und der
  Originalmeldung als Detail.
- Mock (`local/mock/server.mjs`) wirft dieselben `extensions`, `local/realdb/check.mjs` prüft
  Weigerungen über Code und Daten statt über Textteile.
