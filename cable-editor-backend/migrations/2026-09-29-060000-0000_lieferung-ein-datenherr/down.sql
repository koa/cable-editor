drop index lk_lieferung_erstellt_idx;
-- The deliveries so far were of the whole network: the default owner is the closest match
alter table lk_lieferung
    add column eigentuemer_id integer references eigentuemer on delete restrict;
update lk_lieferung
set eigentuemer_id = (select id from eigentuemer where standard);
alter table lk_lieferung
    alter column eigentuemer_id set not null;
create index lk_lieferung_eigentuemer_idx on lk_lieferung (eigentuemer_id, erstellt_am desc);

drop trigger eigentuemer_geaendert on eigentuemer;
alter table eigentuemer
    add column uid varchar(15) unique
        constraint eigentuemer_uid_format check (uid ~ '^(CHE|ZHE)-[0-9]{3}\.[0-9]{3}\.[0-9]{3}$');
create trigger eigentuemer_geaendert
    after update of name, lk_name, uid
    on eigentuemer
    for each row
    when ((old.name, old.lk_name, old.uid) is distinct from (new.name, new.lk_name, new.uid))
execute function eigentuemer_geaendert();
