-- Syncing the circuits of the plan active in Netbox (plan.netbox_active) automatically, see
-- docs/netbox-sync.md: triggers record a change in the transaction that makes it, the backend's
-- worker runs the sync and stores the result.

-- Changes to sync: a row per transaction that changed something the sync reads, inserted by
-- the triggers below. Rows instead of a flag in one row: the transactions don't wait for each
-- other, and a run deletes exactly the rows it saw, so a change during it stays pending.
create table netbox_sync_anstoss
(
    id       bigserial primary key,
    txid     bigint      not null default txid_current(),
    erstellt timestamptz not null default now()
);
create index on netbox_sync_anstoss (txid);

-- The last run, one row, written only by the worker
create table netbox_sync
(
    id                boolean primary key default true check (id),
    letzter_lauf      timestamptz,
    -- null: no run yet
    ergebnis          varchar(10) check (ergebnis in ('ok', 'issues', 'fehler')),
    -- ergebnis 'fehler': the GraphQL error (message, extensions with userError or origin)
    fehler            jsonb,
    -- Failed runs in a row, and when to try again (waiting longer after each)
    fehlversuche      integer not null default 0,
    naechster_versuch timestamptz
);
insert into netbox_sync default values;
-- A run right after the migration
insert into netbox_sync_anstoss default values;

-- The issues of the last run (a serialized SyncIssue each), empty after a successful one
create table netbox_sync_issue
(
    id    serial primary key,
    daten jsonb not null
);

-- One row per transaction: its own rows are visible to it
create function netbox_sync_anstossen_jetzt() returns void
    language sql as
$$
insert into netbox_sync_anstoss (txid)
select txid_current()
where not exists(select 1 from netbox_sync_anstoss where txid = txid_current());
$$;

create function netbox_sync_anstossen() returns trigger
    language plpgsql as
$$
begin
    perform netbox_sync_anstossen_jetzt();
    return null;
end
$$;

-- port_usage: only the rows of the baseline and of the active plan (a planned plan is the
-- baseline and its own rows)
create function netbox_sync_port_usage() returns trigger
    language plpgsql as
$$
declare
    changed_plan integer := coalesce(new.plan_id, old.plan_id);
begin
    if changed_plan = 0 or exists(select 1 from plan where id = changed_plan and netbox_active) then
        perform netbox_sync_anstossen_jetzt();
    end if;
    return null;
end
$$;

create trigger netbox_sync
    after insert or update or delete
    on port_usage
    for each row
execute function netbox_sync_port_usage();

create trigger netbox_sync
    after insert or update or delete
    on panel_port
    for each statement
execute function netbox_sync_anstossen();

create trigger netbox_sync
    after insert or update or delete
    on panel
    for each statement
execute function netbox_sync_anstossen();

create trigger netbox_sync
    after insert or update or delete
    on kabel
    for each statement
execute function netbox_sync_anstossen();

create trigger netbox_sync
    after insert or update or delete
    on kabel_trasse
    for each statement
execute function netbox_sync_anstossen();

-- The length of a cable: its ducts' courses and Schächte (trassen_mit_endpunkten)
create trigger netbox_sync
    after update of geom, schacht_a, schacht_z
    on trasse
    for each statement
execute function netbox_sync_anstossen();

create trigger netbox_sync
    after update of geom
    on schacht
    for each statement
execute function netbox_sync_anstossen();

-- Only a real change: renaming a plan writes all its columns
create trigger netbox_sync
    after update of netbox_active
    on plan
    for each row
    when (old.netbox_active is distinct from new.netbox_active)
execute function netbox_sync_anstossen();
