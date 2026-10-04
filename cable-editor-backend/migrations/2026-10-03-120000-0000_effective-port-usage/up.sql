-- The port usages in force in a plan: its own rows and the rows of the current state (plan 0)
-- it doesn't replace, without the rows removing a fiber (no cable). Every query asking what a
-- port holds in a plan goes through this, so they can't disagree. A plain SQL function, so
-- PostgreSQL inlines it and conditions on its result still use the indexes.
create function effective_port_usage(plan integer)
    returns setof port_usage
    language sql
    stable
as
$$
select usage.*
from port_usage usage
where usage.cable is not null
  and (usage.plan_id = plan
    or (usage.plan_id = 0
        and not exists (select
                        from port_usage planned
                        where planned.plan_id = plan
                          and planned.port_id = usage.port_id
                          and planned.side = usage.side)))
$$;
