-- 3. Port-Verknüpfungen entfernen
ALTER TABLE panel_port
    DROP COLUMN if exists netbox_port_id;

-- 2. Panel-Verknüpfungen entfernen
ALTER TABLE panel
    DROP COLUMN if exists netbox_device_id;

-- 1. Steuerung entfernen
DROP INDEX IF EXISTS idx_plan_single_netbox_active;

ALTER TABLE plan
    DROP COLUMN if exists netbox_active;