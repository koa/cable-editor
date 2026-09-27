# Konzept: Datenlieferung an den Leitungskataster Kanton Zürich

Stand: 27.09.2026 – Konzept, noch nicht umgesetzt. Der heutige Export
(`cable-editor-backend/src/export.rs`) ist ein Platzhalter und wird ersetzt.

## 1. Anforderungen

Grundlage: Weisung LK01-2023 vom 18.08.2023 (auf dem Deckblatt als *Entwurf* bezeichnet)
<https://www.zh.ch/content/dam/zhweb/bilder-dokumente/themen/planen-bauen/geoinformation/kataster/leitungskataster/lk01_weisung_datenlieferung.pdf>,
rechtlich §19 KGeoIG und Leitungskatasterverordnung (LKV, LS 704.14, in Kraft seit 1.5.2022)
<https://www.lexfind.ch/tolv/215462/de>.

- **Modell** `SIA405_LKMap_2015_LV95`, Version `27.04.2018`, INTERLIS 2.3, XTF, LV95.
  Modelldateien (lizenzpflichtig, SIA):
  - <https://405.sia.ch/models/2015/Base_d-20181005.ili>
  - <https://405.sia.ch/models/2015/SIA405_Base_d-20181005.ili>
  - <https://405.sia.ch/models/2015/SIA405_LKMap_2015_2_d-20180427.ili>
- **Termin** (§4 lit. a LKV): innerhalb einer Woche nach Erfassung einer Änderung, mindestens
  am Ende jedes Quartals.
- **Umfang**: pro Medium (hier *Kommunikation*) und Eigentümer immer der ganze Bestand, keine
  Teil- oder inkrementellen Lieferungen. Keine Leitungen anderer Eigentümer („Fremddaten“).
- **Zuständigkeitsperimeter**: Pflicht (§1 Abs. 3 LKV), eigenes Modell
  `Perimeter_LK_ZH_V2_LV95`, Version `2019-04-16` (beim Kanton zu beziehen, die Weisung nennt
  keinen Link). Einmalig vor der ersten LKMap-Lieferung, danach bei Änderungen. Der Checkservice
  warnt, wenn LKMap-Objekte ausserhalb liegen.
- **Dateinamen**: klein geschrieben, in der UID `.` → `-`:
  `<uid-datenherr>-kommunikation-lkmap.xtf` und `<uid-datenherr>-zustaendigkeit-peri.xtf`,
  je in einem ZIP gleichen Namens.
- **Lieferweg**: Upload beim Checkservice von infoGrips
  (<https://www.infogrips.ch/checkservice_login.html>), Zugang über leitungskataster@bd.zh.ch;
  Parameter `data_forward` leitet geprüfte Daten ans Portal weiter, das Prüfresultat kommt per
  Mail.
- **Kontakt**: leitungskataster.support@bd.zh.ch, 043 259 51 33.

### Eigentümer (LKV §1, §4, §8; Weisung 1.2–1.6, 1.9, 4)

- Der Kataster umfasst *alle* Leitungen und Trassen (§1 LKV); lieferpflichtig sind ihre
  Eigentümerinnen und Eigentümer (§4 LKV), eine Ausnahme für Private gibt es nicht. Entlassen
  werden nur einzelne Leitungen in „besonderen Gebieten“ auf begründetes Gesuch (§8 LKV,
  Weisung 4: vertraulich/geheim oder schützenswerte kritische Infrastruktur).
- `Datenherr` = Eigentümer, als UID; `Eigentuemer` = sein Name; `Datenlieferant` = die liefernde
  Stelle, darf eine andere sein (z. B. ein Ingenieurbüro im Auftrag).
- Geliefert wird pro Medium und Eigentümer, Dateiname mit der UID des Datenherrn; pro Eigentümer
  genau eine Zuständigkeitsperimeter-Datei. Die UID muss bei jeder Lieferung dieselbe sein.
- **Fiktive UID** (Weisung 1.6): wer keine UID hat, beantragt bei der Geschäftsstelle eine
  (Format `ZHE-100.100.101`). Genannt sind kleine Flurgenossenschaften und Vereine; natürliche
  Personen erwähnt die Weisung nicht.
- **Fremddaten** (Weisung 1.9): Leitungen anderer Eigentümer dürfen nicht in der eigenen
  Lieferung stecken.
- Einen Eigentümer „privat“ sehen die Vorschriften nicht vor: unter der eigenen UID wäre die
  Trasse im Kataster die eigene (Fremddaten). Möglich ist die Lieferung *als Datenlieferant im
  Auftrag*, in einer eigenen Datei unter der (fiktiven) UID des Eigentümers.

### Pflichtangaben pro Objekt (SIA405_LKMap_2015_LV95)

| Attribut | Inhalt |
|---|---|
| OID (`STANDARDOID`), `OBJ_ID` (TEXT*16) | stabile, eindeutige Objekt-Id |
| `Metaattribute.Datenherr` | UID des Eigentümers (Weisung 1.2) |
| `Metaattribute.Datenlieferant` | UID der liefernden Stelle |
| `Metaattribute.Letzte_Aenderung` | Datum `JJJJMMTT`, unbekannt: `18000101` |
| `Eigentuemer` | Name, TEXT*80 |
| `Lagebestimmung` | `genau` (±10 cm), `ungenau`, `unbekannt` |
| `Status` (optional) | `in_Betrieb`, `ausser_Betrieb`, `tot`, `unbekannt`, `weitere` |
| LKLinie: `Linie`, `Objektart` | Polyline; `Kommunikation.Trasse.unterirdisch` / `.oberirdisch`; optional `Breite` (mm) |
| LKPunkt: `SymbolPos`, `Objektart` | Koordinate; `Kommunikation.Schacht.rund` / `.rechteckig`, `.Bauwerk`, `.Tragwerk`, `.unbekannt`; optional `SymbolOri`, `Dimension1/2` (mm) |

Kabel kommen in LKMap nicht vor, für Kommunikation nur Trassen, Schächte und Bauwerke.

## 2. Entscheide

- **Datenlieferant** (UID) ist konfigurierbar und für alle Daten derselbe.
- **Eigentümer** werden korrekt abgebildet: jede Trasse und jeder Schacht verweist auf einen
  Eigentümer mit Name, Name in der Lieferung und UID (Datenherr). Heute gehört alles einem
  Eigentümer, Dritte (auch Private) kommen hinzu.
- **Lieferung pro Eigentümer mit UID** (als Datenlieferant im Auftrag, wo es nicht der eigene
  ist); Eigentümer ohne UID werden nicht geliefert, die Admin-Seite warnt.
- **Geliefert** werden nur grundstücksübergreifende Verbindungen: pro Trasse ein Schalter,
  Standard **nicht geliefert**. Eine automatische Bestimmung (Grundstücksgrenzen der amtlichen
  Vermessung) ist nicht vorgesehen.
- **Eigenleistung** (entfernt) sagte nur, wer baut, nicht wem die Trasse gehört – für die
  Lieferung ohne Bedeutung.
- **Objektart der Schächte** wird im Schachttyp konfiguriert; heute sind alle rund.
- **`geaendert_am`**: Standard *jetzt*, auch für bestehende Daten (alles ist noch im Bau).
- **Zuständigkeitsperimeter** wird pro Eigentümer berechnet: konvexe Hülle seiner gelieferten
  Trassen und Schächte mit Puffer.
- **Lieferung** zuerst als manueller Download (ZIPs) und manueller Upload beim Checkservice;
  Automatisierung später erwünscht.

## 3. Konfiguration

Neuer Abschnitt in `config.yaml` (überschreibbar als `APP__LKMAP__…`, Helm-Werte dazu):

```yaml
lkmap:
  datenlieferant_uid: "CHE-123.456.789"
  oid_prefix: "ch4711ab"                # 8 Zeichen, Präfix der STANDARDOID
  perimeter_puffer_m: 10                # Puffer um die konvexe Hülle
```

Datenherr und Eigentümer stehen nicht in der Konfiguration, sondern in der Tabelle
`eigentuemer` (siehe 4.).

**OID / `OBJ_ID`** ohne eigene Spalte, stabil aus Präfix, Objektart und Datenbank-Id:
Schacht 42 → `ch4711ab` + `s0000042`, Trasse 7 → `ch4711ab` + `t0000007`.

## 4. Datenbank (eine Migration)

Neue Tabelle `eigentuemer` (Stammdaten):

| Spalte | Typ | Zweck |
|---|---|---|
| `id` | serial | |
| `name` | `text not null unique` | intern, der echte Name |
| `lk_name` | `varchar(80) null` | `Eigentuemer` in der Lieferung, sonst `name` |
| `uid` | `varchar(15) null unique` | `Datenherr`, echte oder fiktive UID; ohne keine Lieferung |

Die Migration legt den heutigen Eigentümer an (Name und UID danach in der Admin-Seite setzen)
und lässt alle bestehenden Schächte und Trassen auf ihn verweisen. Neue Schächte und Trassen
erhalten im UI den zuletzt gewählten bzw. einzigen Eigentümer als Vorschlag.

Neue Enums:
- `genauigkeit`: `genau`, `ungenau`, `unbekannt` (wie SIA405)
- `lkmap_punkt_objektart`: `Schacht_rund`, `Schacht_rechteckig`, `Bauwerk`, `Tragwerk`,
  `unbekannt` (Export: `Kommunikation.Schacht.rund` usw.)

`trasse`:

| Spalte | Typ | Zweck |
|---|---|---|
| `eigentuemer_id` | `integer not null references eigentuemer` | `Datenherr`, `Eigentuemer`, Datei |
| `leitungskataster` | `boolean not null default false` | wird geliefert (grundstücksübergreifend) |
| `lagebestimmung` | `genauigkeit not null default 'unbekannt'` | `Lagebestimmung` |
| `breite_mm` | `integer null`, 0–4000 | optional `Breite` |
| `geaendert_am` | `timestamptz not null default now()` | `Letzte_Aenderung` |

`schacht`:

| Spalte | Typ | Zweck |
|---|---|---|
| `eigentuemer_id` | `integer not null references eigentuemer` | `Datenherr`, `Eigentuemer`, Datei |
| `lagebestimmung` | `genauigkeit not null default 'unbekannt'` | `Lagebestimmung` |
| `geaendert_am` | `timestamptz not null default now()` | `Letzte_Aenderung` |

Ein Schacht wird geliefert, wenn mindestens eine gelieferte Trasse an ihm endet (kein eigener
Schalter), egal wem die Trasse gehört – und zwar in der Datei *seines* Eigentümers: so gibt es
keine gelieferte Trasse ohne ihre Schächte, und keine Fremddaten. Gehört ein solcher Schacht
einem Eigentümer ohne UID, fehlt er in der Lieferung; die Admin-Seite weist darauf hin.

`schacht_typ`:

| Spalte | Typ | Zweck |
|---|---|---|
| `lkmap_objektart` | `lkmap_punkt_objektart not null default 'Schacht_rund'` | `Objektart` des LKPunkt |
| `dimension1_mm`, `dimension2_mm` | `integer null`, 0–4000 | optional `Dimension1/2` |

Trigger für `geaendert_am`:
- Ändern eines Schachts oder einer Trasse setzt `now()`.
- Ändert sich die Position eines Schachts, auch `geaendert_am` seiner Trassen: die gelieferte
  Linie (View `trassen_mit_endpunkten`) beginnt und endet an den Schächten.

Neue Tabelle `lk_lieferung` (Protokoll):

| Spalte | Typ |
|---|---|
| `id` | serial |
| `erstellt_am` | `timestamptz not null default now()` |
| `erstellt_von` | `text not null` (Benutzername) |
| `eigentuemer_id` | `integer not null references eigentuemer` |
| `anzahl_schaechte`, `anzahl_trassen` | integer |
| `geliefert_am` | `timestamptz null` – beim manuellen Upload von Hand bestätigt, später vom automatischen Upload |

Im Export konstant, ohne Spalte:
- `Status` = `in_Betrieb` (Geometrien sind nicht plan-bezogen, es gibt nur Bestehendes)
- Trassen-`Objektart` = `Kommunikation.Trasse.unterirdisch` (oberirdische kennt das System nicht)
- Optional die Kabelzahl pro Trasse als `Eigenschaft` („Anzahl Kabel“), falls gewünscht.

## 5. Export

- Pro Eigentümer mit UID und mindestens einer gelieferten Trasse ein LKMap- und ein
  Perimeter-ZIP; `Datenherr` = seine UID, `Eigentuemer` = `lk_name` bzw. `name`,
  `Datenlieferant` = Konfiguration.
- `SIA405_LKMap_2015_LV95`: Header mit Modell `SIA405_LKMap_2015_LV95`, Version `27.04.2018`,
  URI `http://www.sia.ch/405`; Basket `SIA405_LKMap_2015_LV95.SIA405_LKMap`; `LKPunkt` für
  die gelieferten Schächte (Objektart aus dem Schachttyp), `LKLinie` für die gelieferten
  Trassen mit der Linie aus `trassen_mit_endpunkten`; verschachtelte `Metaattribute`.
- XML über eine Bibliothek schreiben (Escaping), nicht per `format!`.
- `Perimeter_LK_ZH_V2_LV95`: ein Perimeter, Medium `Kommunikation`, Art
  `Zustaendigkeitsperimeter`, Geometrie aus PostGIS:
  `ST_Buffer(ST_ConvexHull(ST_Collect(<Linien der gelieferten Trassen des Eigentümers>)), perimeter_puffer_m)`,
  Bögen als Geraden (`ST_Buffer` mit wenigen Segmenten pro Viertelkreis, z. B. `quad_segs=4`);
  `Letzte_Aenderung` = spätestes `geaendert_am` der gelieferten Trassen des Eigentümers.
- Paketierung: Dateinamen nach Konvention, je ein ZIP.
- Prüfung: `ilivalidator` gegen die Modelle als Test mit Beispieldaten (lokal/CI). Klären, ob
  die lizenzpflichtigen SIA-Modelle ins Repo dürfen oder beim Test geladen werden.

## 6. UI

- Trassen- und Schacht-Eigenschaften: Eigentümer (Auswahl, Planer).
- Eigentümerliste (Admin): Name, Name in der Lieferung, UID; Löschen nur ohne Verweise.
- Trassen-Eigenschaften: Schalter „An Leitungskataster liefern (grundstücksübergreifend)“,
  Lagebestimmung, optional Breite.
- Schacht-Eigenschaften: Lagebestimmung; beim Setzen per GPS Vorschlag aus der Genauigkeit
  (≤ 10 cm → `genau`, sonst `ungenau`).
- Schachttypen: Objektart und Masse – vorerst per Migration (alle `Schacht_rund`), eine
  Bearbeitungsseite bei Bedarf.
- Neue Admin-Seite „Leitungskataster“: pro Eigentümer Download der beiden ZIPs, Warnungen
  (Eigentümer ohne UID mit gelieferten Trassen, Schächte solcher Eigentümer), Liste der Lieferungen,
  „als geliefert markieren“, Hinweise auf Änderungen seit der letzten Lieferung (Wochenfrist)
  und das nahende Quartalsende. Karte des Perimeters und der gelieferten Trassen zur Kontrolle.
- GraphQL: Export und Protokoll mit `RoleGuard(Role::Admin)`.

## 7. Umsetzung (Reihenfolge)

1. Konfiguration `lkmap`, Migration (`eigentuemer`, Enums, Spalten, Trigger, `lk_lieferung`),
   Backend-Felder.
2. UI: Eigentümerliste, Felder bei Trasse, Schacht (und Schachttyp bei Bedarf).
3. LKMap-Export mit Validierungstest.
4. Perimeter-Export.
5. Admin-Seite mit Download und Protokoll.
6. Später: automatischer Upload zum Checkservice (infoGrips dokumentiert nur das Webformular;
   Schnittstelle abklären) und Hinweis per Mail vor Fristen.

## 8. Offen

- Modell `Perimeter_LK_ZH_V2_LV95` beim Kanton beziehen.
- Zugang zum Checkservice beantragen (leitungskataster@bd.zh.ch).
- Pufferbreite des Perimeters (Vorschlag 10 m) mit dem Kanton abstimmen.
- Mit der Katasterleitung klären, wie Privatpersonen ohne UID behandelt werden (fiktive UID
  für natürliche Personen? Pseudonym in `Eigentuemer` zulässig?) und ob die Lieferung ihrer
  Trassen durch uns als Datenlieferant akzeptiert wird.
- Ob private Anschlussleitungen in der Praxis geliefert werden müssen (die LKV macht keine
  Ausnahme).
- Ob die Weisung inzwischen in einer finalen Fassung vorliegt (die geprüfte ist ein Entwurf).
