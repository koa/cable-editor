-- The Kanton wants one delivery per Datenherr: the whole network, whoever owns its parts
-- (docs/leitungskataster.md). The Datenherr is configured, so owners have no UID and a delivery
-- belongs to no owner. What an owner still gives is Eigentuemer, the name in the delivery.
drop trigger eigentuemer_geaendert on eigentuemer;
alter table eigentuemer
    drop column uid;
create trigger eigentuemer_geaendert
    after update of name, lk_name
    on eigentuemer
    for each row
    when ((old.name, old.lk_name) is distinct from (new.name, new.lk_name))
execute function eigentuemer_geaendert();

drop index lk_lieferung_eigentuemer_idx;
alter table lk_lieferung
    drop column eigentuemer_id;
create index lk_lieferung_erstellt_idx on lk_lieferung (erstellt_am desc);
