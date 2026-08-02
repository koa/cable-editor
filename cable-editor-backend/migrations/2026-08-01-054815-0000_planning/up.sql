-- 1. Native Enum für den Plan-Status erstellen
CREATE TYPE plan_status_enum AS ENUM ('Open', 'Implemented', 'Rejected');

-- 2. Tabelle für die Planungen erstellen
CREATE TABLE plan (
                      id serial PRIMARY KEY,
                      name varchar(50) NOT NULL,
                      status plan_status_enum NOT NULL DEFAULT 'Open'
);

-- 3. Der Ist-Zustand (Baseline) bekommt die feste ID 0
INSERT INTO plan (id, name, status) VALUES (0, 'Baseline', 'Implemented');

ALTER TABLE panel_port
    -- Referenz auf die neue 'plan'-Tabelle
    ADD COLUMN plan_id integer NOT NULL DEFAULT 0
        CONSTRAINT fk_panel_port_plan REFERENCES plan(id);

-- Den Primärschlüssel anpassen, damit er die plan_id beinhaltet
ALTER TABLE panel_port DROP CONSTRAINT panel_port_pkey;
ALTER TABLE panel_port ADD PRIMARY KEY (panel_id, port_number, plan_id);


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
    exclude_plan_id     integer := -1;
BEGIN
    -- Wenn es ein UPDATE ist, merken wir uns den alten Primärschlüssel der Zeile
    IF TG_OP = 'UPDATE' THEN
        exclude_panel_id := OLD.panel_id;
        exclude_port_number := OLD.port_number;
        exclude_plan_id := OLD.plan_id;
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

        SELECT count(*) INTO usage_count
        FROM panel_port pp
                 JOIN plan pl ON pp.plan_id = pl.id
        WHERE NOT (pp.panel_id = exclude_panel_id
            AND pp.port_number = exclude_port_number
            AND pp.plan_id = exclude_plan_id)
          -- Filtert auf Ist-Zustand (0) ODER noch offene Planungen
          AND (pl.id = 0 OR pl.status = 'OPEN')
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

        SELECT count(*) INTO usage_count
        FROM panel_port pp
                 JOIN plan pl ON pp.plan_id = pl.id
        WHERE NOT (pp.panel_id = exclude_panel_id
            AND pp.port_number = exclude_port_number
            AND pp.plan_id = exclude_plan_id)
          -- Filtert auf Ist-Zustand (0) ODER noch offene Planungen
          AND (pl.id = 0 OR pl.status = 'OPEN')
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