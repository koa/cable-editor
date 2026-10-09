-- Replaced by the view port_usage_issue below, checked by the backend after every change of a
-- request, which refuses it with a UserError: the triggers checked each row on its own, raised
-- texts the frontend can't word and didn't know where a cable ends.
drop trigger trg_check_port_usage_fasern on port_usage;
drop function check_faser_limits();
drop trigger trg_check_kabel_update on kabel;
drop function check_kabel_update_limits();

-- Where a cable ends: the Schächte only one of its ducts touches (one it passes through is
-- touched by two). The one definition of a cable end, for Schacht.cables and the checks below.
create view kabel_ende as
select kt.kabel, ende.schacht
from kabel_trasse kt
         join trasse t on t.id = kt.trasse
         cross join lateral (values (t.schacht_a), (t.schacht_z)) ende(schacht)
group by kt.kabel, ende.schacht
having count(*) = 1;

create type port_usage_problem_enum as enum ('CableNotEnding', 'FiberOutOfRange', 'FiberTwice');

-- The port usages (of every plan, the baseline too) not fitting the cables, a row per problem:
-- the cable doesn't end in the panel's Schacht, it has no such bundle or fiber, or the same
-- fiber end is at another port side too in the plan (`effective_port_usage`). Mutations refuse
-- to add rows here, admins see and remove the ones there are.
create view port_usage_issue as
select usage.port_id, usage.plan_id, usage.side, usage.cable, usage.bundle, usage.fiber, problem.problem
from port_usage usage
         join panel_port port on port.id = usage.port_id
         join panel on panel.id = port.panel_id
         join kabel on kabel.id = usage.cable
         cross join lateral (select 'CableNotEnding'::port_usage_problem_enum
                             where not exists (select
                                               from kabel_ende ende
                                               where ende.kabel = usage.cable
                                                 and ende.schacht = panel.schacht_id)
                             union all
                             select 'FiberOutOfRange'
                             where usage.bundle not between 1 and kabel.buendel_anz
                                or usage.fiber not between 1 and kabel.faser_anz
                             union all
                             select 'FiberTwice'
                             where exists (select
                                           from effective_port_usage(usage.plan_id) other
                                                    join panel_port other_port on other_port.id = other.port_id
                                                    join panel other_panel on other_panel.id = other_port.panel_id
                                           where other.cable = usage.cable
                                             and other.bundle = usage.bundle
                                             and other.fiber = usage.fiber
                                             and other_panel.schacht_id = panel.schacht_id
                                             and (other.port_id, other.side) <> (usage.port_id, usage.side))) problem(problem);
