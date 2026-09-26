create type plan_status_enum as enum ('Open', 'Implemented', 'Rejected');
alter table plan
    add column status plan_status_enum not null default 'Open';
update plan
set status = 'Implemented'
where id = 0;
