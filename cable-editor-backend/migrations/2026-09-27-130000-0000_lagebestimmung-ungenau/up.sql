-- The Lagebestimmung is ungenau unless set explicitly (docs/leitungskataster.md). The existing
-- unbekannt all come from the previous default, nobody could set it yet.
alter table schacht
    alter column lagebestimmung set default 'ungenau';
alter table trasse
    alter column lagebestimmung set default 'ungenau';

-- Not a change of the objects: keep their Letzte_Aenderung
alter table schacht
    disable trigger schacht_geaendert_am;
alter table trasse
    disable trigger trasse_geaendert_am;
update schacht
set lagebestimmung = 'ungenau'
where lagebestimmung = 'unbekannt';
update trasse
set lagebestimmung = 'ungenau'
where lagebestimmung = 'unbekannt';
alter table schacht
    enable trigger schacht_geaendert_am;
alter table trasse
    enable trigger trasse_geaendert_am;
