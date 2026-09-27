-- Owners of Schächte and ducts: the Leitungskataster is delivered per owner (Datenherr = UID,
-- see docs/leitungskataster.md).
create table eigentuemer
(
    id       serial primary key,
    name     text    not null unique,
    -- Eigentuemer in the delivery, 'Keine_Angabe' if the name isn't released; null: name
    lk_name  varchar(80),
    -- real or fictitious UID (Weisung 1.5/1.6); without one the owner isn't delivered
    uid      varchar(15) unique
        constraint eigentuemer_uid_format check (uid ~ '^(CHE|ZHE)-[0-9]{3}\.[0-9]{3}\.[0-9]{3}$'),
    -- the owner of new Schächte and ducts unless given
    standard boolean not null default false
);
create unique index eigentuemer_single_standard on eigentuemer (standard) where standard;

-- Everything belongs to one owner so far; name and UID are set on the admin page
insert into eigentuemer (name, standard)
values ('Eigentümer', true);

alter table schacht
    add column eigentuemer_id integer references eigentuemer on delete restrict;
alter table trasse
    add column eigentuemer_id integer references eigentuemer on delete restrict;
update schacht
set eigentuemer_id = (select id from eigentuemer where standard);
update trasse
set eigentuemer_id = (select id from eigentuemer where standard);
alter table schacht
    alter column eigentuemer_id set not null;
alter table trasse
    alter column eigentuemer_id set not null;
create index schacht_eigentuemer_idx on schacht (eigentuemer_id);
create index trasse_eigentuemer_idx on trasse (eigentuemer_id);

-- An insert without owner gets the standard one (a column default can't be a query)
create function eigentuemer_standard() returns trigger
    language plpgsql as
$$
begin
    if new.eigentuemer_id is null then
        select id into new.eigentuemer_id from eigentuemer where standard;
    end if;
    return new;
end
$$;
create trigger schacht_eigentuemer_standard
    before insert
    on schacht
    for each row
execute function eigentuemer_standard();
create trigger trasse_eigentuemer_standard
    before insert
    on trasse
    for each row
execute function eigentuemer_standard();
