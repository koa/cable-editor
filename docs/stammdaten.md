# Konzept: Seiten für Eigentümer und Schachttypen

Stand: 27.09.2026 – Konzept; umgesetzt sind die Eigentümer (bis auf die Spalte in der Trassenliste). Was davon der Leitungskataster braucht
(UID, Name in der Lieferung, Objektart, Masse), steht in `docs/leitungskataster.md`; wo die
Seiten liegen, in `docs/navigation.md`.

## Eigentümer (umgesetzt)

Eigentümer von Schächten und Trassen (Tabelle `eigentuemer`, GraphQL `Owner`, `listOwner`).

- Seite `PlanView::ListOfOwners`, Bereich „Eigentümer“; für alle lesbar, ändern nur Admin.
- Tabelle: Name, Name in der Lieferung, UID, Standard, Anzahl Schächte und Trassen (davon
  geliefert).
- Anlegen und Bearbeiten in einem Dialog (drei Felder), keine eigene Seite; die Aktionen im
  Menü der Zeile.
- „Als Standard setzen“: der Standard-Eigentümer wird neuen Schächten und Trassen vorgewählt;
  es gibt genau einen.
- Löschen nur ohne Schächte und Trassen und nicht den Standard-Eigentümer; sonst nennt die
  Meldung, was noch auf ihn verweist.
- Trassenliste: Spalte „Eigentümer“.

GraphQL (Admin): `createOwner`, `updateOwner` (`OwnerInput`: `name`, `lkName`, `uid`; Name
getrimmt und nicht leer, UID mit `config::is_uid`, doppelte UID mit klarer Meldung),
`setDefaultOwner` (setzt `standard` in einer Transaktion um), `deleteOwner`; an `Owner` die
Zählfelder `schachtCount`, `ductCount`, `deliveredDuctCount` über den Loader.

## Schachttypen

Typen von Schächten (Tabelle `schacht_typ`, GraphQL `SchachtTyp`).

- Seiten `PlanView::ListOfCabinetTypes`, `PlanView::CabinetType { id }` und
  `PlanView::NewCabinetType { id }` (wie ein neuer Schacht, `IdOrNew`), Bereich „Schachttyp“;
  für alle lesbar, ändern nur Admin.
- Felder: Name (max. 20 Zeichen), Icon und die Felder für den Leitungskataster.
- Icon: aus einer SVG-Datei; die Vorschau als `<img>` mit `data:`-URL, damit ein Skript im SVG
  nicht läuft.
- Die Liste zeigt die Anzahl Schächte; Löschen nur ohne Schächte.

GraphQL (Admin): `createSchachtTyp`, `updateSchachtTyp`, `deleteSchachtTyp`.

## Umsetzung

Je ein Commit für Eigentümer und für Schachttypen: Mutationen, Seite, Mock
(`local/mock/server.mjs`); geprüft mit `local/realdb/check.mjs` (Änderungen und Weigerungen)
und Screenshots (`local/mock/screenshot.mjs`, Handy und Desktop).
