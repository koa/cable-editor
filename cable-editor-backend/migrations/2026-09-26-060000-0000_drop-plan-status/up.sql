-- Plans are deleted when implemented and there is no rejecting, so every plan but the
-- baseline (id 0, the current state) was 'Open': the status only restated whether a plan
-- is the baseline, which GraphQL now exposes as Plan.isBaseline.
alter table plan drop column status;
drop type plan_status_enum;
