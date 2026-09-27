-- What the delivery to the Leitungskataster (SIA405 LKMap) needs, see docs/leitungskataster.md.
create type genauigkeit_enum as enum ('genau', 'ungenau', 'unbekannt');
create type lkmap_punkt_objektart_enum as enum
    ('Schacht_rund', 'Schacht_rechteckig', 'Bauwerk', 'Tragwerk', 'unbekannt');

alter table schacht_typ
    add column lkmap_objektart lkmap_punkt_objektart_enum not null default 'Schacht_rund',
    add column dimension1_mm   integer check (dimension1_mm between 0 and 4000),
    add column dimension2_mm   integer check (dimension2_mm between 0 and 4000),
    add constraint schacht_typ_dimension_order check (dimension2_mm <= dimension1_mm);

alter table schacht
    add column lagebestimmung genauigkeit_enum not null default 'unbekannt',
    add column geaendert_am   timestamptz      not null default now();

alter table trasse
    -- delivered (connections across plots), chosen per duct
    add column leitungskataster boolean          not null default false,
    add column lagebestimmung   genauigkeit_enum not null default 'unbekannt',
    add column breite_mm        integer check (breite_mm between 0 and 4000),
    add column geaendert_am     timestamptz      not null default now();

-- Letzte_Aenderung: every change of a row ...
create function geaendert_am_setzen() returns trigger
    language plpgsql as
$$
begin
    new.geaendert_am := now();
    return new;
end
$$;
create trigger schacht_geaendert_am
    before update
    on schacht
    for each row
    when (old.* is distinct from new.*)
execute function geaendert_am_setzen();
create trigger trasse_geaendert_am
    before update
    on trasse
    for each row
    when (old.* is distinct from new.*)
execute function geaendert_am_setzen();

-- ... and of what the delivered objects take from elsewhere: a duct's line starts and ends at
-- its Schächte (view trassen_mit_endpunkten), ...
create function schacht_verschoben() returns trigger
    language plpgsql as
$$
begin
    update trasse set geaendert_am = now() where schacht_a = new.id or schacht_z = new.id;
    return null;
end
$$;
create trigger schacht_verschoben
    after update of geom
    on schacht
    for each row
    when (old.geom is distinct from new.geom)
execute function schacht_verschoben();

-- ... a Schacht's Objektart and dimensions come from its type, ...
create function schacht_typ_geaendert() returns trigger
    language plpgsql as
$$
begin
    update schacht set geaendert_am = now() where typ = new.id;
    return null;
end
$$;
create trigger schacht_typ_geaendert
    after update of lkmap_objektart, dimension1_mm, dimension2_mm
    on schacht_typ
    for each row
    when ((old.lkmap_objektart, old.dimension1_mm, old.dimension2_mm)
        is distinct from (new.lkmap_objektart, new.dimension1_mm, new.dimension2_mm))
execute function schacht_typ_geaendert();

-- ... and Eigentuemer and Datenherr from the owner
create function eigentuemer_geaendert() returns trigger
    language plpgsql as
$$
begin
    update schacht set geaendert_am = now() where eigentuemer_id = new.id;
    update trasse set geaendert_am = now() where eigentuemer_id = new.id;
    return null;
end
$$;
create trigger eigentuemer_geaendert
    after update of name, lk_name, uid
    on eigentuemer
    for each row
    when ((old.name, old.lk_name, old.uid) is distinct from (new.name, new.lk_name, new.uid))
execute function eigentuemer_geaendert();

-- Deliveries, one row per exported file set of an owner
create table lk_lieferung
(
    id               serial primary key,
    eigentuemer_id   integer     not null references eigentuemer on delete restrict,
    erstellt_am      timestamptz not null default now(),
    erstellt_von     text        not null,
    anzahl_schaechte integer     not null,
    anzahl_trassen   integer     not null,
    -- sha256 of the LKMap objects: tells changes since, deletions included
    pruefsumme       char(64)    not null,
    -- confirmed by hand after the upload, later by the automatic upload
    geliefert_am     timestamptz
);
create index lk_lieferung_eigentuemer_idx on lk_lieferung (eigentuemer_id, erstellt_am desc);
