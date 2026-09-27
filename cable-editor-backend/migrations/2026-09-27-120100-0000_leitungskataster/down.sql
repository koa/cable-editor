drop table lk_lieferung;
drop trigger eigentuemer_geaendert on eigentuemer;
drop function eigentuemer_geaendert();
drop trigger schacht_typ_geaendert on schacht_typ;
drop function schacht_typ_geaendert();
drop trigger schacht_verschoben on schacht;
drop function schacht_verschoben();
drop trigger trasse_geaendert_am on trasse;
drop trigger schacht_geaendert_am on schacht;
drop function geaendert_am_setzen();
alter table trasse
    drop column geaendert_am,
    drop column breite_mm,
    drop column lagebestimmung,
    drop column leitungskataster;
alter table schacht
    drop column geaendert_am,
    drop column lagebestimmung;
alter table schacht_typ
    drop constraint schacht_typ_dimension_order,
    drop column dimension2_mm,
    drop column dimension1_mm,
    drop column lkmap_objektart;
drop type lkmap_punkt_objektart_enum;
drop type genauigkeit_enum;
