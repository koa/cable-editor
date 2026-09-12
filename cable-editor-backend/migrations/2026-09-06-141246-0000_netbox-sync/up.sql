-- 1. Steuerung: Welcher Plan wird gerade nach Netbox synchronisiert?
ALTER TABLE plan
    ADD COLUMN netbox_active BOOLEAN NOT NULL DEFAULT FALSE;

-- Partieller Unique-Index: Nur maximal ein Plan darf TRUE sein
CREATE UNIQUE INDEX idx_plan_single_netbox_active
    ON plan (netbox_active)
    WHERE netbox_active = TRUE;

-- Die Baseline (ID 0) als initialen NetBox-Zustand setzen
UPDATE plan
SET netbox_active = TRUE
WHERE id = 0;

-- 2. Verknüpfung Panel -> NetBox Device
ALTER TABLE panel
    ADD COLUMN netbox_device_id INTEGER;

-- 3. Verknüpfung Port -> NetBox Port (FrontPort, RearPort, Interface)
ALTER TABLE panel_port
    ADD COLUMN netbox_port_id INTEGER;