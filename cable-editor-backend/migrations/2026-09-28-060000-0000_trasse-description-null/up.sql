-- An empty description showed as an empty name instead of the duct's Schächte
-- (duct_title). The mutations store none since they trim; this clears older rows.

-- Not a change of the objects: keep their Letzte_Aenderung
alter table trasse
    disable trigger trasse_geaendert_am;
update trasse
set description = null
where btrim(description) = '';
alter table trasse
    enable trigger trasse_geaendert_am;

alter table trasse
    add constraint trasse_description_not_empty
        check (description is null or btrim(description) <> '');
