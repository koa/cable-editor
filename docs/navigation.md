# Konzept: Navigation

Stand: 27.09.2026.

- Alle Seiten liegen im Pfad hinter dem Plan (`/plan/<id>/…`), auch solche für Daten, die
  nicht plan-bezogen sind (Stammdaten wie Schächte, Trassen, Eigentümer, Schachttypen): so
  lässt sich jedes Element direkt verlinken. Der Plan ist eher eine globale Einstellung der UI,
  die beim Wechseln die aktuelle Ansicht behält.
- Jede Art von Element ist ein Bereich im Bereichsmenü des Breadcrumbs („Planung bearbeiten“,
  „Karte“, „Schacht“, „Kabel“, „Trasse“, „Eigentümer“, „Schachttyp“, „Leitungskataster“,
  „Netbox“), nach Zweck gruppiert in Planung, Objekte, Stammdaten und Lieferung; neue Arten
  kommen als weitere Bereiche dazu.
- Jedes Menü im Breadcrumb bietet genau eine Ebene an, und genau ein Eintrag ist aktiv: das
  Menü einer Elementart nur die Elemente (z. B. die Schächte), das nächste Menü die Ansichten
  des Elements darüber und die Elemente eine Ebene tiefer (bei einem Schacht seine Ansichten
  und Root-Panels, bei einem Panel seine Ansichten und Unterpanels). Auf einer Ansicht ist
  die Ansicht aktiv, auf einem Panel das Panel.
