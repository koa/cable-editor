-- This file should undo anything in `up.sql`

drop index if exists idx_panel_port_f1;
drop index if exists idx_panel_port_f2;

DROP TRIGGER IF EXISTS trg_check_kabel_update ON kabel;

DROP TABLE IF EXISTS panel_port;
DROP TYPE IF EXISTS port_type_enum;
DROP TABLE IF EXISTS panel;

DROP FUNCTION IF EXISTS check_faser_limits();
DROP FUNCTION IF EXISTS check_kabel_update_limits();