-- 1. Index entfernen
drop index if exists idx_port_usage;

-- 2. Trigger entfernen
drop trigger if exists trg_check_kabel_update on kabel;
drop trigger if exists trg_check_port_usage_fasern on port_usage;

-- 3. Trigger-Funktionen entfernen
drop function if exists check_kabel_update_limits();
drop function if exists check_faser_limits();

-- 4. Tabellen in umgekehrter Reihenfolge der Abhängigkeiten entfernen
drop table if exists port_usage;
drop table if exists plan;
drop table if exists panel_port;
drop table if exists panel;

-- 5. Native Enums (Types) entfernen
drop type if exists port_side_enum;
drop type if exists plan_status_enum;
drop type if exists port_type_enum;