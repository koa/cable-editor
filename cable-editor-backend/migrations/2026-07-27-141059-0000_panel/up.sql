create table panel
(
    id           serial
        constraint panel_pk primary key,

    name         varchar(20),
    schacht_id   integer not null
        constraint panel_schacht_id_fk references schacht,
    parent_panel integer
        constraint panel_parent_id_fk references panel,
    parent_order integer,
    constraint panel_unique_parent_order unique (parent_panel, parent_order),
    constraint check_parent_order_required check (parent_panel is null or parent_order is not null)
);

create type port_type_enum as enum ('Splice', 'Connector','Loop');

create table panel_port
(
    id         serial primary key,
    panel_id   integer        not null
        constraint panel_port_panel_id_fk references panel,

    port_order integer        not null,
    label      varchar(20),
    port_type  port_type_enum not null,

    constraint panel_port_unique unique (panel_id, port_order),
    constraint check_valid_port_type check (port_type in ('Splice', 'Connector', 'Loop')),
    constraint check_port_number_positive check (port_order >= 0)
);

-- 1. Native Enum für den Plan-Status erstellen
create type plan_status_enum as enum ('Open', 'Implemented', 'Rejected');

-- 2. Tabelle für die Planungen erstellen
create table plan
(
    id     serial primary key,
    name   varchar(50)      not null,
    status plan_status_enum not null default 'Open'
);

-- 3. Der Ist-Zustand (Baseline) bekommt die feste ID 0
insert into plan (id, name, status)
values (0, 'Baseline', 'Implemented');


create type port_side_enum as enum ('Front', 'Back');
create table port_usage
(
    port_id integer        not null
        constraint fk_belegung_port references panel_port (id),
    plan_id integer        not null
        constraint fk_belegung_plan references plan (id),
    side    port_side_enum not null,

    -- Fasern (wie bisher)
    cable   integer
        constraint belegung_f1_kabel_fk references kabel (id),
    fiber   integer,
    bundle  integer,

    primary key (port_id, plan_id, side)

);



create or replace function check_faser_limits()
    returns trigger as
$$
declare
    k_buendel_anz     integer;
    k_faser_anz       integer;
    usage_count       integer;
    other_side_cable  integer;
    other_side_bundle integer;
    other_side_fiber  integer;
begin
    -- Nur prüfen, wenn auch wirklich ein Kabel zugewiesen wird (Tombstones werden ignoriert)
    if NEW.cable is not null then

        -- 1. Kabel-Limits aus der Tabelle 'kabel' laden
        select buendel_anz, faser_anz
        into k_buendel_anz, k_faser_anz
        from kabel
        where id = NEW.cable;

        if NEW.bundle < 1 or NEW.bundle > k_buendel_anz then
            raise exception 'Bündel % ist ungültig (Kabel erlaubt max. %)', NEW.bundle, k_buendel_anz;
        end if;

        if NEW.fiber < 1 or NEW.fiber > k_faser_anz then
            raise exception 'Faser % ist ungültig (Kabel erlaubt max. %)', NEW.fiber, k_faser_anz;
        end if;

        -- 2. Plausibilität: Gleiche Faser darf nicht auf Front und Back desselben Ports liegen (Loop)
        select cable, bundle, fiber
        into other_side_cable, other_side_bundle, other_side_fiber
        from port_usage
        where port_id = NEW.port_id
          and plan_id = NEW.plan_id
          and side != NEW.side;

        if FOUND and other_side_cable = NEW.cable and other_side_bundle = NEW.bundle and
           other_side_fiber = NEW.fiber then
            raise exception 'Fehler: Eine Faser kann nicht auf Front und Back desselben Ports (Loop) aufgelegt werden.';
        end if;

        -- 3. Globale Limitierung pro PLAN: Faser darf im effektiven Plan max. 2 Mal (Start & Ende) existieren
        select count(*)
        into usage_count
        from (
                 -- Teil A: Alle Belegungen im AKTUELLEN Plan
                 select port_id, side, cable, bundle, fiber
                 from port_usage
                 where plan_id = NEW.plan_id
                   -- Die aktuell bearbeitete Zeile ausschließen (da sie durch NEW repräsentiert wird)
                   and not (port_id = NEW.port_id and side = NEW.side)

                 union all

                 -- Teil B: Alle Belegungen aus der BASELINE (plan_id = 0)...
                 select port_id, side, cable, bundle, fiber
                 from port_usage p0
                 where p0.plan_id = 0
                   and NEW.plan_id != 0 -- ...nur auswerten, wenn wir gerade in einem Plan > 0 arbeiten
                   -- ...und auch nur dann übernehmen, wenn dieser Port in unserem aktuellen Plan NICHT verändert/geleert wurde
                   and not exists (select 1
                                   from port_usage px
                                   where px.plan_id = NEW.plan_id
                                     and px.port_id = p0.port_id
                                     and px.side = p0.side)
                   -- Auch hier den bearbeiteten Port ausschließen
                   and not (p0.port_id = NEW.port_id and
                            p0.side = NEW.side)) as effective_plan
        where cable = NEW.cable
          and bundle = NEW.bundle
          and fiber = NEW.fiber;

        if usage_count >= 2 then
            raise exception 'Faser % (Bündel %) von Kabel % ist im effektiven Zustand von Plan % bereits an 2 Enden belegt!', NEW.fiber, NEW.bundle, NEW.cable, NEW.plan_id;
        end if;

    end if;

    return NEW;
end;
$$ language plpgsql;

-- Den Trigger an die neue Tabelle hängen:
create trigger trg_check_port_usage_fasern
    before insert or update
    on port_usage
    for each row
execute function check_faser_limits();

-- 1. Die Trigger-Funktion für das Kabel definieren
create or replace function check_kabel_update_limits()
    returns trigger as
$$
begin
    -- Prüfung 1: Wurde die Bündel-Anzahl verkleinert?
    if NEW.buendel_anz < OLD.buendel_anz then
        -- Schauen, ob irgendwo ein Bündel > dem NEUEN Maximalwert genutzt wird
        if exists (select 1
                   from port_usage
                   where cable = NEW.id
                     and bundle > NEW.buendel_anz) then
            raise exception 'Fehler: Bündel-Anzahl kann nicht auf % reduziert werden. Es sind bereits höhere Bündel auf Ports gepatcht.', NEW.buendel_anz;
        end if;
    end if;

    -- Prüfung 2: Wurde die Faser-Anzahl verkleinert?
    if NEW.faser_anz < OLD.faser_anz then
        -- Schauen, ob irgendwo eine Faser > dem NEUEN Maximalwert genutzt wird
        if exists (select 1
                   from port_usage
                   where cable = NEW.id
                     and fiber > NEW.faser_anz) then
            raise exception 'Fehler: Faser-Anzahl kann nicht auf % reduziert werden. Es sind bereits höhere Fasern auf Ports gepatcht.', NEW.faser_anz;
        end if;
    end if;

    return NEW;
end;
$$ language plpgsql;

-- 2. Den Trigger an die Tabelle 'kabel' hängen
create trigger trg_check_kabel_update
    before update
    on kabel
    for each row
execute function check_kabel_update_limits();

create index idx_port_usage on port_usage (cable, bundle, fiber);