
drop table if exists plan cascade ;

DROP TYPE IF EXISTS plan_status_enum cascade ;

ALTER TABLE panel_port DROP CONSTRAINT panel_port_pkey;
ALTER TABLE panel_port ADD PRIMARY KEY (panel_id, port_number);
alter table panel_port drop column plan_id;



CREATE OR REPLACE FUNCTION check_faser_limits()
    RETURNS TRIGGER AS
$$
DECLARE
    k_buendel_anz       integer;
    k_faser_anz         integer;
    usage_count         integer;

    -- Variablen zum Ausschließen der eigenen Zeile beim Update
    exclude_panel_id    integer := -1;
    exclude_port_number integer := -1;
BEGIN
    -- Wenn es ein UPDATE ist, merken wir uns den alten Primärschlüssel der Zeile
    IF TG_OP = 'UPDATE' THEN
        exclude_panel_id := OLD.panel_id;
        exclude_port_number := OLD.port_number;
    END IF;

    -- 1. Plausibilitätsprüfung: Faser 1 und Faser 2 am selben Port dürfen nicht identisch sein
    IF NEW.f1_kabel_id IS NOT NULL AND NEW.f2_kabel_id IS NOT NULL THEN
        IF NEW.f1_kabel_id = NEW.f2_kabel_id AND
           NEW.f1_buendel = NEW.f2_buendel AND
           NEW.f1_faser = NEW.f2_faser THEN
            RAISE EXCEPTION 'Fehler: Eine Faser kann nicht zweimal am selben Port aufgelegt werden.';
        END IF;
    END IF;

    -- Prüfung für Faser 1 (falls vorhanden)
    IF NEW.f1_kabel_id IS NOT NULL THEN
        SELECT buendel_anz, faser_anz
        INTO k_buendel_anz, k_faser_anz
        FROM kabel
        WHERE id = NEW.f1_kabel_id;

        IF NEW.f1_buendel < 1 OR NEW.f1_buendel > k_buendel_anz THEN
            RAISE EXCEPTION 'Faser 1: Bündel % ist ungültig (Kabel erlaubt max. %)', NEW.f1_buendel, k_buendel_anz;
        END IF;

        IF NEW.f1_faser < 1 OR NEW.f1_faser > k_faser_anz THEN
            RAISE EXCEPTION 'Faser 1: Faser % ist ungültig (Kabel erlaubt max. %)', NEW.f1_faser, k_faser_anz;
        END IF;

        SELECT count(*)
        INTO usage_count
        FROM panel_port
        WHERE NOT (panel_id = exclude_panel_id AND port_number = exclude_port_number) -- Ignoriert die eigene Zeile beim Update
          AND (
            (f1_kabel_id = NEW.f1_kabel_id AND f1_buendel = NEW.f1_buendel AND f1_faser = NEW.f1_faser)
                OR (f2_kabel_id = NEW.f1_kabel_id AND f2_buendel = NEW.f1_buendel AND f2_faser = NEW.f1_faser)
            );

        IF usage_count >= 2 THEN
            RAISE EXCEPTION 'Faser % (Bündel %) von Kabel % ist bereits an 2 anderen Ports belegt!', NEW.f1_faser, NEW.f1_buendel, NEW.f1_kabel_id;
        END IF;
    END IF;

    -- Prüfung für Faser 2 (falls vorhanden)
    IF NEW.f2_kabel_id IS NOT NULL THEN
        SELECT buendel_anz, faser_anz
        INTO k_buendel_anz, k_faser_anz
        FROM kabel
        WHERE id = NEW.f2_kabel_id;

        IF NEW.f2_buendel < 1 OR NEW.f2_buendel > k_buendel_anz THEN
            RAISE EXCEPTION 'Faser 2: Bündel % ist ungültig (Kabel erlaubt max. %)', NEW.f2_buendel, k_buendel_anz;
        END IF;

        IF NEW.f2_faser < 1 OR NEW.f2_faser > k_faser_anz THEN
            RAISE EXCEPTION 'Faser 2: Faser % ist ungültig (Kabel erlaubt max. %)', NEW.f2_faser, k_faser_anz;
        END IF;

        SELECT count(*)
        INTO usage_count
        FROM panel_port
        WHERE NOT (panel_id = exclude_panel_id AND port_number = exclude_port_number) -- Ignoriert die eigene Zeile beim Update
          AND (
            (f1_kabel_id = NEW.f2_kabel_id AND f1_buendel = NEW.f2_buendel AND f1_faser = NEW.f2_faser)
                OR (f2_kabel_id = NEW.f2_kabel_id AND f2_buendel = NEW.f2_buendel AND f2_faser = NEW.f2_faser)
            );

        IF usage_count >= 2 THEN
            RAISE EXCEPTION 'Faser % (Bündel %) von Kabel % ist bereits an 2 anderen Ports belegt!', NEW.f2_faser, NEW.f2_buendel, NEW.f2_kabel_id;
        END IF;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql