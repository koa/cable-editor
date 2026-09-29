# Konzept: Datenlieferung an den Leitungskataster Kanton Zürich

Stand: 29.09.2026 – umgesetzt bis zum manuellen Upload beim Checkservice: die Datenbank mit
den Feldern in GraphQL (Abschnitt 4), die Konfiguration (Abschnitt 3), der Export beider
Dateien als ZIP mit dem Protokoll der Lieferungen (Abschnitt 5, `cable-editor-backend/src/lkmap/`)
und die UI mit der Admin-Seite (Abschnitt 6, `pages/lkmap.rs`). Offen ist der automatische
Upload (Abschnitt 7, Schritt 7).

Umstellung beschlossen, noch nicht umgesetzt (Abschnitt 7, Schritt 6): der Kanton wünscht
entgegen den Weisungen *eine* Lieferung pro Datenherr, also eine für das ganze Netz, egal wem
die einzelnen Teile gehören. Dieses Dokument beschreibt bereits den Zielzustand; die
Abschnitte 4 bis 6 gelten für den Code erst nach Schritt 6.

## 1. Anforderungen

Grundlage: Weisung LK01-2023 vom 18.08.2023 (auf dem Deckblatt als *Entwurf* bezeichnet)
<https://www.zh.ch/content/dam/zhweb/bilder-dokumente/themen/planen-bauen/geoinformation/kataster/leitungskataster/lk01_weisung_datenlieferung.pdf>,
rechtlich §19 KGeoIG und Leitungskatasterverordnung (LKV, LS 704.14, in Kraft seit 1.5.2022)
<https://www.lexfind.ch/tolv/215462/de>.

- **Unterlagen** (Weisungen, Modelle, Werkkontakte) verlinkt der Kanton auf
  <https://www.zh.ch/de/planen-bauen/geoinformation/kataster/leitungskataster.html>
  (dort Stand 27.09.2026: LK01 vom 18.08.2023, LK02 Zugangs- und Nutzungsbestimmungen vom
  10.02.2026).
- **Modell** `SIA405_LKMap_2015_LV95`, Version `27.04.2018`, INTERLIS 2.3, XTF, LV95.
  Die Modelldateien des SIA gehören nicht ins (öffentliche) Repo: das LKMap-Modell ist
  lizenzpflichtig, alle drei stehen unter Copyright des SIA. Sie liegen auf:
  - <https://405.sia.ch/models/2015/Base_d-20181005.ili> (`Base`, `Base_LV95`, 05.10.2018)
  - <https://405.sia.ch/models/2015/SIA405_Base_d-20181005.ili> (`SIA405_Base`,
    `SIA405_Base_LV95`, 05.10.2018)
  - <https://405.sia.ch/models/2015/SIA405_LKMap_2015_2_d-20180427.ili> (`SIA405_LKMap_2015`,
    `SIA405_LKMap_2015_LV95`, 27.04.2018)

  Dazu `Units` (INTERLIS-Standardmodell, bringt ilivalidator mit). Was der Export daraus
  braucht, steht unten.
- **Termin** (§4 lit. a LKV): innerhalb einer Woche nach Erfassung einer Änderung, mindestens
  am Ende jedes Quartals.
- **Umfang**: pro Medium (hier *Kommunikation*) und Datenherr immer der ganze Bestand, keine
  Teil- oder inkrementellen Lieferungen.
- **Zuständigkeitsperimeter**: Pflicht (§1 Abs. 3 LKV), eigenes Modell
  `Perimeter_LK_ZH_V2_LV95`, Version `2019-04-16` (vom Kanton, ohne Lizenzvorbehalt, im Repo:
  `docs/perimeter_lk_zh_v2_lv95.zip`, siehe unten). Einmalig vor der ersten LKMap-Lieferung,
  danach bei Änderungen. Der Checkservice warnt, wenn LKMap-Objekte ausserhalb liegen.
- **Dateinamen**: klein geschrieben, in der UID des Datenherrn `.` → `-`:
  `<uid-datenherr>-kommunikation-lkmap.xtf` und `<uid-datenherr>-zustaendigkeit-peri.xtf`,
  je in einem ZIP gleichen Namens.
- **Lieferweg**: Upload beim Checkservice von infoGrips
  (<https://www.infogrips.ch/checkservice_login.html>), Zugang über leitungskataster@bd.zh.ch;
  Parameter `data_forward` leitet geprüfte Daten ans Portal weiter, das Prüfresultat kommt per
  Mail.
- **Kontakt**: leitungskataster.support@bd.zh.ch, 043 259 51 33.

### Datenherr und Eigentümer (LKV §1, §4, §8; Weisung 1.2–1.6, 1.9, 4)

- Der Kataster umfasst *alle* Leitungen und Trassen (§1 LKV); lieferpflichtig sind ihre
  Eigentümerinnen und Eigentümer (§4 LKV), eine Ausnahme für Private gibt es nicht. Entlassen
  werden nur einzelne Leitungen in „besonderen Gebieten“ auf begründetes Gesuch (§8 LKV,
  Weisung 4: vertraulich/geheim oder schützenswerte kritische Infrastruktur).
- Nach den Weisungen wäre `Datenherr` der Eigentümer jeder Leitung (als UID), geliefert pro
  Medium und Eigentümer, `Datenlieferant` die liefernde Stelle (z. B. ein Ingenieurbüro im
  Auftrag). Wer keine UID hat, beantragt eine fiktive bei der Geschäftsstelle (Weisung 1.6,
  Format `ZHE-100.100.101`; genannt sind kleine Flurgenossenschaften und Vereine).
- **Abweichend davon wünscht der Kanton Zürich** (Mitteilung an uns, 29.09.2026) eine Lieferung
  pro Datenherr. Für uns heisst das: *eine* Lieferung für das ganze Netz, unabhängig davon, was
  dem Verein und was den Grundeigentümern gehört. `Datenherr` ist die UID des Vereins, der
  das Netz betreibt (bei Bedarf eine andere als die des Datenlieferanten), und der Dateiname
  trägt diese UID; es gibt genau eine LKMap- und eine Zuständigkeitsperimeter-Datei.
- Wem ein Schacht oder eine Trasse gehört, steht im Attribut `Eigentuemer` jedes Objekts
  (Name, `TEXT*80`). Die Eigentümer brauchen deshalb keine UID.
- **Fremddaten** (Weisung 1.9) gibt es damit nicht mehr: alle gelieferten Objekte gehören zum
  Netz des Datenherrn, auch wenn sie einem Grundeigentümer gehören.
- **Name nicht freigegeben**: das Modell sieht dafür in `Eigentuemer` den Text `Keine_Angabe`
  vor (Kommentar in `LKObjekt`).

### Modell `Perimeter_LK_ZH_V2_LV95`

`docs/perimeter_lk_zh_v2_lv95.zip` enthält `Perimeter_LK_ZH_V2_LV95.ILI` (INTERLIS 2.3,
Latin-1). Modell-URI `http://models.geo.zh.ch`, Version `2019-04-16`; importiert `Units`,
`Base_LV95` und `SIA405_Base_LV95` – zum Prüfen braucht es also auch die SIA-Basismodelle.
Topic `Perimeter_LK_ZH` (Basket `Perimeter_LK_ZH_V2_LV95.Perimeter_LK_ZH`), eine Klasse
`Perimeter`, ohne Metaattribute-Struktur und ohne `OBJ_ID`:

| Attribut | Typ | Inhalt für uns |
|---|---|---|
| `Medium` | Pflicht, Aufzählung `Abwasser` … `Kommunikation` … `weitereMedien` | `Kommunikation` |
| `Datenherr` | Pflicht, `TEXT*15` (UID) | `datenherr_uid` aus der Konfiguration |
| `Datenlieferant` | Pflicht, `TEXT*15` (UID) | `datenlieferant_uid` aus der Konfiguration |
| `Letzte_Aenderung` | Pflicht, `INTERLIS_1_DATE` (`JJJJMMTT`) | letzte Änderung, sonst Datum des Exports |
| `Art` | Pflicht, `Zustaendigkeitsperimeter`, `Projektperimeter`, `Perimeter_entlassenes_Gebiet`, `Perimeter_eingeschraenkte_Nutzung` | `Zustaendigkeitsperimeter` (die beiden letzten erfasst die Katasterleitung) |
| `Begleitdokument` | optional, `URI` | leer |
| `Geometrie` | Pflicht, `Base_LV95.Surface` | die Hülle mit Puffer |

`Letzte_Aenderung`: laut Weisung `18000101`, wenn unbekannt; laut Modellkommentar das Datum
des Exports. Bei uns ist sie immer bekannt (`geaendert_am`).

`Base_LV95.Surface` ist `SURFACE WITH (STRAIGHTS, ARCS) VERTEX LKoord WITHOUT OVERLAPS > 0.050`:
eine *einzelne* Fläche (Aussenrand, allenfalls Löcher), obwohl der Kommentar im Modell
„Multifläche“ sagt. Mehrere Teilflächen wären mehrere `Perimeter`-Objekte; die konvexe Hülle
ist immer eine Fläche. Da `Medium` pro Objekt steht, gehören alle Medien eines Datenherrn in
dieselbe Datei (Weisung 1.4) – für uns nur `Kommunikation`.

### Modell `SIA405_LKMap_2015_LV95` (Auszug für Kommunikation)

Topic `SIA405_LKMap` (Basket `SIA405_LKMap_2015_LV95.SIA405_LKMap`), Modell-URI
`http://www.sia.ch/405`. Klassen `LKPunkt`, `LKLinie`, `LKFlaeche` (alle abgeleitet von
`LKObjekt`) und `LKObjekt_Text` (Beschriftung, optional, per Komposition an ein `LKObjekt`).

Allen gemeinsam (`SIA405_Base_LV95.SIA405_BaseClass`, `LKObjekt`):

| Attribut | Typ | Inhalt für uns |
|---|---|---|
| OID (TID) | `STANDARDOID` (16 Zeichen: 8 Präfix + 8) | `oid_prefix` + `s`/`t` + 7 Ziffern |
| `OBJ_ID` | `TEXT*16`, optional, `UNIQUE` | dieselbe Id wie die OID |
| `Metaattribute` | Pflicht, Struktur `SIA405_Base_LV95.Metaattribute` | |
| ↳ `Datenherr` | Pflicht, `OrganisationBezeichnung` (`TEXT*80`) | UID des Datenherrn (Weisung 1.2) |
| ↳ `Datenlieferant` | Pflicht, `TEXT*80` | UID des Datenlieferanten |
| ↳ `Letzte_Aenderung` | Pflicht, `INTERLIS_1_DATE` (`JJJJMMTT`) | `geaendert_am` |
| `Eigentuemer` | Pflicht, `TEXT*80` | Eigentümer des Objekts: `lk_name` bzw. `name`, nicht freigegeben: `Keine_Angabe` |
| `Lagebestimmung` | Pflicht, `genau` (±10 cm, aus verschiedenen Messungen ±30 cm), `ungenau`, `unbekannt` | Spalte `lagebestimmung` |
| `Status` | optional, `ausser_Betrieb`, `in_Betrieb`, `tot`, `unbekannt`, `weitere` | `in_Betrieb` |
| `Eigenschaft` | optional, `BAG OF Eigenschaften` (`Bezeichnung`, `Wert`, je `TEXT*80`) | allenfalls Anzahl Kabel |

`LKPunkt` (Schacht):

| Attribut | Typ | Inhalt für uns |
|---|---|---|
| `SymbolPos` | Pflicht, `Base_LV95.LKoord` | Position des Schachts |
| `SymbolOri` | optional, `0.0 .. 359.9` Grad | leer (Standard 90°) |
| `Dimension1`, `Dimension2` | optional, `0 .. 4000` mm (grösseres/kleineres Innenmass) | aus dem Schachttyp |
| `Objektart` | Pflicht; Kommunikation: `Kommunikation.Bauwerk`, `Kommunikation.Schacht.rechteckig`, `Kommunikation.Schacht.rund`, `Kommunikation.Tragwerk`, `Kommunikation.unbekannt` | aus dem Schachttyp |

`LKLinie` (Trasse):

| Attribut | Typ | Inhalt für uns |
|---|---|---|
| `Linie` | Pflicht, `Base_LV95.Polyline` (`POLYLINE WITH (STRAIGHTS, ARCS) VERTEX LKoord`) | aus `trassen_mit_endpunkten` |
| `Objektart` | Pflicht; Kommunikation: `Kommunikation.Trasse.oberirdisch`, `Kommunikation.Trasse.unterirdisch` | `Kommunikation.Trasse.unterirdisch` |
| `Breite` | optional, `0 .. 4000` mm (ab 300 mm als Doppellinie gezeichnet; breiter als 4 m als Fläche) | `breite_mm` |
| `Profiltyp` | optional, `Eiprofil` … `unbekannt`, `weitere` | leer (bisher nur Abwasser) |

`LKFlaeche` (Kommunikation: `Bauwerk`, `Schacht`, `Tragwerk`, `Trasse`, `unbekannt`, Geometrie
`Base_LV95.Surface`) ist für Bauwerke mit detaillierter Geometrie; wir liefern keine.

Geometrie: `LKoord` = `COORD 2480000.000 .. 2840000.000, 1070000.000 .. 1300000.000` (LV95,
3 Nachkommastellen, also auf mm runden); Linien ungerichtet, Bögen erlaubt (wir liefern nur
Geraden).

Kabel kommen in LKMap nicht vor, für Kommunikation nur Trassen, Schächte und Bauwerke.

## 2. Entscheide

- **Datenlieferant** und **Datenherr** (je eine UID) sind konfigurierbar und für alle Daten
  dieselben; ohne eigene Angabe ist der Datenherr der Datenlieferant.
- **Eine Lieferung für das ganze Netz** (Wunsch des Kantons, siehe 1.): eine LKMap- und eine
  Perimeter-Datei, Dateiname mit der UID des Datenherrn.
- **Eigentümer** werden weiterhin abgebildet: jede Trasse und jeder Schacht verweist auf einen
  Eigentümer mit Name und Name in der Lieferung (`Eigentuemer`). Heute gehört alles einem
  Eigentümer, Dritte (auch Private) kommen hinzu. Eine UID pro Eigentümer gibt es nicht.
- **Geliefert** werden nur grundstücksübergreifende Verbindungen: pro Trasse ein Schalter,
  Standard **nicht geliefert**. Eine automatische Bestimmung (Grundstücksgrenzen der amtlichen
  Vermessung) ist nicht vorgesehen.
- **Eigenleistung** (entfernt) sagte nur, wer baut, nicht wem die Trasse gehört – für die
  Lieferung ohne Bedeutung.
- **Objektart der Schächte** wird im Schachttyp konfiguriert; heute sind alle rund.
- **Lagebestimmung** ist standardmässig `ungenau`, ausser sie wird ausdrücklich anders gesetzt.
- **`geaendert_am`**: Standard *jetzt*, auch für bestehende Daten (alles ist noch im Bau).
- **Zuständigkeitsperimeter** ein einziger für das ganze Netz: konvexe Hülle der gelieferten
  Trassen und Schächte mit Puffer.
- **Lieferung** zuerst als manueller Download (ZIPs) und manueller Upload beim Checkservice;
  Automatisierung später erwünscht.

## 3. Konfiguration

Neuer Abschnitt in `config.yaml` (überschreibbar als `APP__LKMAP__…`, Helm-Werte dazu):

```yaml
lkmap:
  datenlieferant_uid: "CHE-123.456.789"
  datenherr_uid: "CHE-987.654.321"      # optional, sonst datenlieferant_uid
  oid_prefix: "ch4711ab"                # 8 Zeichen, Präfix der STANDARDOID
  perimeter_puffer_m: 10                # Puffer um die konvexe Hülle
```

Die Eigentümer stehen nicht in der Konfiguration, sondern in der Tabelle `eigentuemer`
(siehe 4.).

Umgesetzt in `config.rs` (`LKMAP_CONFIG`): der Abschnitt ist optional (ohne ihn kein Export,
beim Start ein Hinweis im Log), ein ungültiger (eine UID nicht im Format `CHE-`/`ZHE-123.456.789`,
Präfix nicht 8 Buchstaben oder Ziffern mit einem Buchstaben vorne, Puffer nicht positiv)
verhindert den Start. Helm: `config.lkmap` (`datenlieferantUid`, `datenherrUid`, `oidPrefix`,
`perimeterPufferM`), ohne `datenlieferantUid` kein Abschnitt.

**OID / `OBJ_ID`** ohne eigene Spalte, stabil aus Präfix, Objektart und Datenbank-Id:
Schacht 42 → `ch4711ab` + `s0000042`, Trasse 7 → `ch4711ab` + `t0000007`.
Der Basket (`BID`) jeder Datei erhält eine OID nach demselben Schema: LKMap `b0000001`,
Perimeter `p0000001`.

Das Präfix wird **zentral vergeben**, bestellt per Webformular auf
<https://www.interlis.ch/dienste/oid-bestellen>. Regelung: INTERLIS 2.3 Referenzhandbuch, Anhang D
„Aufbau von Objektidentifikatoren (OID)“
(<https://www.interlis.ch/download/interlis2/ili2-refman_2006-04-13_d.pdf>, S. 126 ff.; im
Referenzhandbuch 2.4 Anhang F):
- OID = Präfix (8 Zeichen) + Postfix (8 Zeichen), nur Buchstaben und Ziffern.
- Präfix: erstes Zeichen ein Buchstabe, die ersten zwei der Ländercode nach ISO 3166 (`ch`), die
  übrigen sechs „von einer zuständigen, zentralen Stelle einmalig vergeben“.
- Postfix: verwaltet der Datenproduzent selbst, freie Stellen links mit `0` aufgefüllt.
- Zum Üben: `ch100000` für Objekte, `chB00000` für Baskets.
- Der Anhang ist ausdrücklich nicht normativ, sondern ein „Standard-Erweiterungsvorschlag … im
  Sinne einer Empfehlung“; normativ ist nur `STANDARDOID = OID TEXT*16`. Die Weisung LK01
  sagt zu OIDs nichts.
- Laut Anhang ist „typischerweise für jeden Behälter … ein neues Präfix erforderlich“. Da unsere
  OIDs über alle Dateien eindeutig sind (Präfix + Objektart + Datenbank-Id), genügt ein Präfix
  für die ganze Datenbank.

`config.rs` prüft das Präfix nach Anhang D (8 Buchstaben oder Ziffern, vorne ein Buchstabe);
`ch` am Anfang wird nicht erzwungen, da nur empfohlen.

## 4. Datenbank (umgesetzt)

Migrationen in `cable-editor-backend/migrations`: `…_eigentuemer` (Stammdaten, auch ohne
Leitungskataster sinnvoll), `…_leitungskataster` und `…_lieferung-ein-datenherr` (die
Umstellung auf eine Lieferung für das ganze Netz: die Spalten `eigentuemer.uid` und
`lk_lieferung.eigentuemer_id` entfallen, bestehende Protokolleinträge bleiben); Schema in `src/db/schema.rs`, die Enums
als `db/entity/lkmap.rs` (`Genauigkeit`, `LkmapPunktObjektart`). Geprüft mit PostgreSQL 16 und
PostGIS 3.6 an den Beispieldaten (`local/data.sql`), auch down und wieder up.

Neue Tabelle `eigentuemer` (Stammdaten):

| Spalte | Typ | Zweck |
|---|---|---|
| `id` | serial | |
| `name` | `text not null unique` | intern, der echte Name |
| `lk_name` | `varchar(80) null` | `Eigentuemer` der Objekte in der Lieferung, sonst `name`; `Keine_Angabe`, wenn nicht freigegeben |
| `standard` | `boolean not null default false`, höchstens einer (partieller Unique-Index) | Eigentümer neuer Schächte und Trassen |

Die Migration legt den heutigen Eigentümer als Standard an (`Eigentümer`, den Namen danach
in der Admin-Seite setzen) und lässt alle bestehenden Schächte und Trassen auf ihn verweisen.
Ein Insert ohne `eigentuemer_id` erhält den Standard-Eigentümer (Trigger, ein Spalten-Default
kann keine Abfrage sein); so bleiben `createSchacht`/`createDuct` ohne Eigentümer gültig.

Neue Enums:
- `genauigkeit_enum`: `genau`, `ungenau`, `unbekannt` (wie SIA405)
- `lkmap_punkt_objektart_enum`: `Schacht_rund`, `Schacht_rechteckig`, `Bauwerk`, `Tragwerk`,
  `unbekannt` (Export: `LkmapPunktObjektart::transfer_value`, `Kommunikation.Schacht.rund` usw.)

`trasse`:

| Spalte | Typ | Zweck |
|---|---|---|
| `eigentuemer_id` | `integer not null references eigentuemer` | `Eigentuemer` |
| `leitungskataster` | `boolean not null default false` | wird geliefert (grundstücksübergreifend) |
| `lagebestimmung` | `genauigkeit_enum not null default 'ungenau'` (erst `unbekannt`, siehe 7.) | `Lagebestimmung` |
| `breite_mm` | `integer null`, 0–4000 | optional `Breite` |
| `geaendert_am` | `timestamptz not null default now()` | `Letzte_Aenderung` |

`schacht`:

| Spalte | Typ | Zweck |
|---|---|---|
| `eigentuemer_id` | `integer not null references eigentuemer` | `Eigentuemer` |
| `lagebestimmung` | `genauigkeit_enum not null default 'ungenau'` (erst `unbekannt`, siehe 7.) | `Lagebestimmung` |
| `geaendert_am` | `timestamptz not null default now()` | `Letzte_Aenderung` |

Ein Schacht wird geliefert, wenn mindestens eine gelieferte Trasse an ihm endet (kein eigener
Schalter), egal wem Schacht und Trasse gehören: so gibt es keine gelieferte Trasse ohne ihre
Schächte.

`schacht_typ`:

| Spalte | Typ | Zweck |
|---|---|---|
| `lkmap_objektart` | `lkmap_punkt_objektart_enum not null default 'Schacht_rund'` | `Objektart` des LKPunkt |
| `dimension1_mm`, `dimension2_mm` | `integer null`, 0–4000, `dimension2_mm <= dimension1_mm` | optional `Dimension1/2` |

Trigger für `geaendert_am` (`now()`, also der Beginn der Transaktion):
- Ändern eines Schachts oder einer Trasse, nur wenn sich die Zeile wirklich ändert
  (`old.* is distinct from new.*`): Speichern ohne Änderung ist keine Änderung.
- Ändert sich die Position eines Schachts, auch seine Trassen: die gelieferte Linie (View
  `trassen_mit_endpunkten`) beginnt und endet an den Schächten.
- Ändern von `lkmap_objektart` oder den Massen eines Schachttyps: seine Schächte.
- Ändern von `name` oder `lk_name` eines Eigentümers: seine Schächte und Trassen
  (`Eigentuemer`).

Neue Tabelle `lk_lieferung` (Protokoll):

| Spalte | Typ |
|---|---|
| `id` | serial |
| `erstellt_am` | `timestamptz not null default now()` |
| `erstellt_von` | `text not null` (Benutzername) |
| `anzahl_schaechte`, `anzahl_trassen` | `integer not null` |
| `pruefsumme` | `char(64) not null` – SHA-256 der beiden Dateien |
| `geliefert_am` | `timestamptz null` – beim manuellen Upload von Hand bestätigt, später vom automatischen Upload |

Ob sich seit der letzten Lieferung etwas geändert hat, sagt der Vergleich der Prüfsumme mit
der des aktuellen Exports – `geaendert_am` allein erkennt gelöschte Trassen nicht.

Rust-Seite: die Spalten sind Felder von `Duct`, `Schacht` und `SchachtTyp`, `eigentuemer` die
Entität `Eigentuemer` (`db/entity/eigentuemer.rs`, über den Loader `EigentuemerId`); die
Zeitstempel als `chrono::DateTime<Utc>` (Feature `chrono` von diesel und async-graphql).
GraphQL (lesend): `listOwner`, `Owner { id name lkName isDefault }`, bei `Duct` `owner`,
`leitungskataster`, `lagebestimmung`, `widthMm`, `changedAt`, bei `Schacht` `owner`,
`lagebestimmung`, `changedAt`, bei `SchachtTyp` `lkmapObjektart`, `dimension1Mm`,
`dimension2Mm`. Die Mutationen dazu kommen mit der UI (7., Schritt 2).

Im Export konstant, ohne Spalte:
- `Status` = `in_Betrieb` (Geometrien sind nicht plan-bezogen, es gibt nur Bestehendes)
- Trassen-`Objektart` = `Kommunikation.Trasse.unterirdisch` (oberirdische kennt das System nicht)
- Optional die Kabelzahl pro Trasse als `Eigenschaft` („Anzahl Kabel“), falls gewünscht.

## 5. Export

- Für das ganze Netz ein LKMap- und ein Perimeter-ZIP; `Datenherr` = `datenherr_uid`,
  `Datenlieferant` = `datenlieferant_uid`, `Eigentuemer` jedes Objekts = `lk_name` bzw. `name`
  seines Eigentümers.
- `SIA405_LKMap_2015_LV95`: Header mit Modell `SIA405_LKMap_2015_LV95`, Version `27.04.2018`,
  URI `http://www.sia.ch/405`; Basket `SIA405_LKMap_2015_LV95.SIA405_LKMap`; `LKPunkt` für
  die gelieferten Schächte (Objektart aus dem Schachttyp), `LKLinie` für die gelieferten
  Trassen mit der Linie aus `trassen_mit_endpunkten`; `Metaattribute` als verschachtelte
  Struktur (`<Metaattribute><SIA405_Base_LV95.Metaattribute>…`), Aufzählungen mit Punkt
  (`Kommunikation.Schacht.rund`).
- XML über eine Bibliothek schreiben (Escaping), nicht per `format!` (umgesetzt mit
  `quick-xml`, `lkmap/xtf.rs`; die Attribute in der Reihenfolge des Modells, ein einzelner Punkt
  wie `SymbolPos` ebenfalls in `COORD`).
- Umgesetzt (`lkmap/mod.rs`, GraphQL `lkmapExport`, nur Admin):
  - Geliefert wird eine Trasse mit gesetztem Schalter `leitungskataster`, ein Schacht, wenn eine
    solche Trasse an ihm endet (egal wem sie gehört).
  - Eine Trasse ohne eigenen Verlauf ist eine gerade Linie zwischen ihren Schächten und wird mit
    `Lagebestimmung` `unbekannt` geliefert, unabhängig vom gespeicherten Wert (die Trassen-Seite
    zeigt das so an).
  - Ein Schacht ohne Position wird nicht geliefert, ebenso eine gelieferte Trasse, die an ihm
    endet; beide meldet der Export als Bericht (`schaechteWithoutPosition`, `ductsWithoutLine`).
  - Keine `Eigenschaft` (auch nicht die Anzahl Kabel).
  - Verweigert (`UserError`): ohne Abschnitt `lkmap`, nichts zu liefern, eine Id mit mehr als
    7 Stellen (OID).
- `Perimeter_LK_ZH_V2_LV95` (Header: Modell `Perimeter_LK_ZH_V2_LV95`, Version `2019-04-16`,
  URI `http://models.geo.zh.ch`; Basket `Perimeter_LK_ZH_V2_LV95.Perimeter_LK_ZH`): ein
  `Perimeter`, Medium `Kommunikation`, Art `Zustaendigkeitsperimeter`,
  `Datenherr`/`Datenlieferant` wie im LKMap, Geometrie aus PostGIS als `SURFACE`
  (`BOUNDARY`/`POLYLINE`/`COORD` mit `C1`/`C2`, auf mm gerundet):
  `ST_Buffer(ST_ConvexHull(ST_Collect(<Linien der gelieferten Trassen>)), perimeter_puffer_m)`,
  Bögen als Geraden (`ST_Buffer` mit wenigen Segmenten pro Viertelkreis, z. B. `quad_segs=4`);
  `Letzte_Aenderung` = spätestes `geaendert_am` der gelieferten Trassen.
  Umgesetzt (`lkmap/perimeter.rs`, in `lkmapExport` als `perimeter`): die Hülle genau der
  gelieferten Objekte (Linien der Trassen und Positionen der Schächte), `Letzte_Aenderung` das
  späteste ihrer Daten, die Koordinaten auf mm gerundet ohne doppelte Punkte; BID
  `<prefix>p0000001`, TID `<prefix>z0000001` (das Modell hat keine OID).
- Paketierung (umgesetzt): je ein ZIP gleichen Namens mit der XTF-Datei
  (`<uid-datenherr>-kommunikation-lkmap.zip`, `<uid-datenherr>-zustaendigkeit-peri.zip`), gebaut vom Backend
  (`TransferFile::zip`) und von `downloadLkmap` in Base64 geliefert.
- Prüfsumme: SHA-256 über beide Dateien. Sie enthalten kein Exportdatum, gleiche Daten ergeben
  also gleiche Dateien; so erkennt der Vergleich auch gelöschte Objekte und einen geänderten
  Puffer.
- Protokoll (umgesetzt): `downloadLkmap` schreibt beim Herunterladen einen Eintrag in
  `lk_lieferung` (wer, wann, Anzahl, Prüfsumme); ein noch nicht bestätigter Eintrag mit derselben
  Prüfsumme wird wiederverwendet. `setLkmapDelivered(deliveryId, delivered)` setzt
  `geliefert_am` nach dem Upload beim Checkservice oder nimmt es zurück (z. B. wenn der
  Checkservice die Dateien ablehnt). Verweigert wird der Download ohne etwas
  mit Position (`NothingToDeliver`); `lkmapExport` liefert in diesem Fall den Bericht ohne
  Dateien.
- Frist: `firstChangeSinceDelivery` ist das früheste `geaendert_am` eines gelieferten Objekts nach
  dem Herunterladen der zuletzt bestätigten Lieferung (Beginn der Wochenfrist); fehlt es trotz
  anderer Prüfsumme (nur gelöscht), ist die Lieferung ohne Datum fällig.
- Prüfung: Unit-Tests für die Datei (`cargo test -p cable-editor-backend lkmap`) und
  `ilivalidator` gegen die Modelle mit `local/lkmap/validate.sh <datei.xtf>` (lädt ilivalidator
  1.15.0 ins Cache-Verzeichnis, Java aus dem PATH oder von nix; die Modelle holt ilivalidator
  selbst aus den Repositories `https://405.sia.ch/models` und `https://models.geo.zh.ch`, nichts
  davon kommt ins Repo, siehe 1.). `local/realdb/check.mjs` exportiert echte Daten und prüft
  sie damit. Vorerst nur lokal, in CI später (braucht Java und Netzwerk).

## 6. UI

Was die Lieferung in der UI braucht; die Seiten für Eigentümer und Schachttypen selbst beschreibt
`docs/stammdaten.md`.

- **Eigentümer**: Name in der Lieferung („Name nicht freigeben“ setzt `Keine_Angabe`)
  bearbeiten. Ändern nur Admin, da der Name als `Eigentuemer` in die Lieferung geht.
- **Schachttyp**: Objektart (`LKPunkt.Objektart`) und Masse (Dimension 1 ≥ Dimension 2, je
  0–4000 mm) bearbeiten; ändern nur Admin.
- **Trassen-Eigenschaften**: Eigentümer (Planer; neu: der Standard-Eigentümer), Schalter „An
  Leitungskataster liefern (grundstücksübergreifend)“, Lagebestimmung, optional Breite (mm); die
  Trassen-Seite zeigt sie mit „geändert am“.
- **Trassenliste**: Spalte „LK“ (wird geliefert).
- **Schacht-Eigenschaften**: Eigentümer, Lagebestimmung.
- **Lagebestimmung**: Standard immer `ungenau`, ausser man setzt sie ausdrücklich anders; kein
  automatischer Vorschlag (auch nicht aus der GPS-Genauigkeit).
- **Admin-Seite „Leitungskataster“** (umgesetzt, `pages/lkmap.rs`, Bereich im Pfad hinter dem
  Plan): die Lieferung für das ganze Netz mit Inhalt (Trassen, Schächte), Datenherr, Zustand und
  Download der beiden ZIPs. Zustand, dringendster zuerst: nichts zu liefern, noch nie geliefert,
  geändert seit der letzten bestätigten Lieferung (liefern bis eine Woche nach der ersten
  Änderung, danach rot), in diesem Quartal nicht geliefert (bis Quartalsende, ab 14 Tagen davor
  als Warnung), aktuell. Warnungen (Schächte ohne Position, Trassen an solchen) mit Links, die
  Lieferungen mit „Als geliefert bestätigen“ bzw. „Bestätigung zurücknehmen“. Darunter eine
  Karte mit dem Perimeter und den gelieferten Trassen und Schächten.
- GraphQL: Export und Protokoll mit `RoleGuard(Role::Admin)`; Eigentümer, Lieferung,
  Lagebestimmung und Breite einer Trasse oder eines Schachts setzen Planer (`DuctInput`,
  `SchachtInput`, Pflichtfelder).

## 7. Umsetzung (Reihenfolge)

1. ~~Migration (`eigentuemer`, Enums, Spalten, Trigger, `lk_lieferung`), Diesel-Schema~~
   (erledigt); ~~Konfiguration `lkmap`, Backend-Felder (lesend)~~ (erledigt).
2. ~~UI (Abschnitt 6)~~ (erledigt):
   1. ~~Migration: Lagebestimmung standardmässig `ungenau` (bestehende `unbekannt` stammen alle
      vom Standardwert und werden `ungenau`, ohne `geaendert_am` zu verschieben)~~ (erledigt).
   2. ~~Eigentümer- und Schachttyp-Seiten (`docs/stammdaten.md`)~~ (erledigt).
   3. ~~Trassen: Felder in `DuctInput`, Eigenschaften, Trassen-Seite, Spalte „LK“ der Liste~~
      (erledigt).
   4. ~~Schächte: Felder in `SchachtInput`, Eigenschaften~~ (erledigt).
3. ~~LKMap-Export mit Validierungstest~~ (erledigt).
4. ~~Perimeter-Export~~ (erledigt).
5. ~~Admin-Seite mit Download und Protokoll~~ (erledigt).
6. Eine Lieferung pro Datenherr statt pro Eigentümer (Wunsch des Kantons, siehe 1.):
   `datenherr_uid` in der Konfiguration, `eigentuemer.uid` und `lk_lieferung.eigentuemer_id`
   entfallen, ein Export und ein Perimeter für das ganze Netz, die Admin-Seite mit einer
   Lieferung.
7. Später: automatischer Upload zum Checkservice (infoGrips dokumentiert nur das Webformular;
   Schnittstelle abklären) und Hinweis per Mail vor Fristen.

## 8. Offen

- Zugang zum Checkservice beantragen (leitungskataster@bd.zh.ch).
- UID des Datenherrn (des Vereins) klären, falls sie nicht die des Datenlieferanten ist, und mit
  der Katasterleitung abstimmen, ob `Keine_Angabe` in `Eigentuemer` akzeptiert wird.
- OID-Präfix bestellen (<https://www.interlis.ch/dienste/oid-bestellen>, siehe 3.); `ch4711ab`
  ist ein Platzhalter.
- Pufferbreite des Perimeters (Vorschlag 10 m) mit dem Kanton abstimmen.
- Ob private Anschlussleitungen in der Praxis geliefert werden müssen (die LKV macht keine
  Ausnahme).
- Ob die Weisung inzwischen in einer finalen Fassung vorliegt (die geprüfte ist ein Entwurf).
