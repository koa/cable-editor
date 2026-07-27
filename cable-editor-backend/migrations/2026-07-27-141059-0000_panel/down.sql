-- This file should undo anything in `up.sql`

DROP TRIGGER IF EXISTS trg_check_kabel_update ON kabel;

DROP TABLE IF EXISTS panel_port;
DROP TABLE IF EXISTS panel;

DROP FUNCTION IF EXISTS check_faser_limits();
DROP FUNCTION IF EXISTS check_kabel_update_limits();