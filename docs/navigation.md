# Konzept: Navigation

Stand: 27.09.2026.

- Alle Seiten liegen im Pfad hinter dem Plan (`/plan/<id>/…`), auch solche für Daten, die
  nicht plan-bezogen sind (Stammdaten wie Schächte, Trassen, Eigentümer, Schachttypen): so
  lässt sich jedes Element direkt verlinken. Der Plan ist eher eine globale Einstellung der UI,
  die beim Wechseln die aktuelle Ansicht behält.
- Jede Art von Element ist ein Bereich im Bereichsmenü des Breadcrumbs (heute „Ändern“,
  „Schacht“, „Kabel“, „Trasse“, „Karte“); neue Arten (z. B. „Eigentümer“, „Schachttyp“)
  kommen als weitere Bereiche dazu.
