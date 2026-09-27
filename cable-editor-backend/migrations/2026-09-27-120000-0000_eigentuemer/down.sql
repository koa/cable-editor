drop trigger trasse_eigentuemer_standard on trasse;
drop trigger schacht_eigentuemer_standard on schacht;
drop function eigentuemer_standard();
alter table trasse
    drop column eigentuemer_id;
alter table schacht
    drop column eigentuemer_id;
drop table eigentuemer;
