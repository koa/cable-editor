-- Your SQL goes here
create table panel
(
    id           serial
        constraint panel_pk primary key,

    name         varchar(20),
    schacht_id   integer not null
        constraint panel_schacht_id_fk references schacht,
    parent_panel integer
        constraint panel_parent_id_fk references panel,
    parent_order integer,
    CONSTRAINT panel_unique_parent_order UNIQUE (parent_panel, parent_order),
    CONSTRAINT check_parent_order_required CHECK (parent_panel IS NULL OR parent_order IS NOT NULL)
);

CREATE TABLE panel_port
(
    id          serial
        constraint panel_port_pk primary key,

    panel_id    integer     not null
        constraint panel_port_panel_id_fk references panel,

    port_number integer     not null,
    label       varchar(20),
    port_type   varchar(20) not null,

    f1_kabel_id integer
        CONSTRAINT port_f1_kabel_fk REFERENCES kabel (id),
    f1_buendel  integer,
    f1_faser    integer,

    f2_kabel_id integer
        CONSTRAINT port_f2_kabel_fk REFERENCES kabel (id),
    f2_buendel  integer,
    f2_faser    integer,

    CONSTRAINT panel_port_unique_number UNIQUE (panel_id, port_number),
    CONSTRAINT check_valid_port_type CHECK (port_type IN ('Splice', 'Connector')),
    CONSTRAINT check_port_number_positive CHECK (port_number > 0),
    CONSTRAINT check_port_belegung CHECK (
        -- FALL 1: Port ist komplett unbelegt
        (
            f1_kabel_id IS NULL AND f1_buendel IS NULL AND f1_faser IS NULL AND
            f2_kabel_id IS NULL AND f2_buendel IS NULL AND f2_faser IS NULL
            )
            OR
            -- FALL 2: Port ist ein 'Connector' -> Genau Faser 1 muss belegt sein
        (
            port_type = 'Connector' AND
            f1_kabel_id IS NOT NULL AND f1_buendel IS NOT NULL AND f1_faser IS NOT NULL AND
            f2_kabel_id IS NULL AND f2_buendel IS NULL AND f2_faser IS NULL
            )
            OR
            -- FALL 3: Port ist ein 'Splice' -> Faser 1 UND Faser 2 müssen belegt sein
        (
            port_type = 'Splice' AND
            f1_kabel_id IS NOT NULL AND f1_buendel IS NOT NULL AND f1_faser IS NOT NULL AND
            f2_kabel_id IS NOT NULL AND f2_buendel IS NOT NULL AND f2_faser IS NOT NULL
            )
        )
);

CREATE OR REPLACE FUNCTION check_faser_limits()
    RETURNS TRIGGER AS
$$
DECLARE
    k_buendel_anz integer;
    k_faser_anz   integer;
    usage_count   integer;
BEGIN
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
        -- a) Existieren Bündel/Faser auf dem Kabel?
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

        -- b) [NEU] Ist die Faser bereits 2x belegt?
        SELECT count(*)
        INTO usage_count
        FROM panel_port
        WHERE id IS DISTINCT FROM NEW.id -- Wichtig für Updates, damit wir die eigene Zeile nicht mitzählen
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
        -- a) Existieren Bündel/Faser auf dem Kabel?
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

        -- b) [NEU] Ist die Faser bereits 2x belegt?
        SELECT count(*)
        INTO usage_count
        FROM panel_port
        WHERE id IS DISTINCT FROM NEW.id
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
$$ LANGUAGE plpgsql;

-- 2. Den Trigger an die Tabelle hängen
CREATE TRIGGER trg_check_panel_port_fasern
    BEFORE INSERT OR UPDATE
    ON panel_port
    FOR EACH ROW
EXECUTE FUNCTION check_faser_limits();

-- 1. Die Trigger-Funktion für das Kabel definieren
CREATE OR REPLACE FUNCTION check_kabel_update_limits()
    RETURNS TRIGGER AS
$$
BEGIN
    -- Prüfung 1: Wurde die Bündel-Anzahl verkleinert?
    IF NEW.buendel_anz < OLD.buendel_anz THEN
        -- Schauen, ob irgendwo ein Bündel > dem NEUEN Maximalwert genutzt wird
        IF EXISTS (SELECT 1
                   FROM panel_port
                   WHERE (f1_kabel_id = NEW.id AND f1_buendel > NEW.buendel_anz)
                      OR (f2_kabel_id = NEW.id AND f2_buendel > NEW.buendel_anz)) THEN
            RAISE EXCEPTION 'Fehler: Bündel-Anzahl kann nicht auf % reduziert werden. Es sind bereits höhere Bündel auf Ports gepatcht.', NEW.buendel_anz;
        END IF;
    END IF;

    -- Prüfung 2: Wurde die Faser-Anzahl verkleinert?
    IF NEW.faser_anz < OLD.faser_anz THEN
        -- Schauen, ob irgendwo eine Faser > dem NEUEN Maximalwert genutzt wird
        IF EXISTS (SELECT 1
                   FROM panel_port
                   WHERE (f1_kabel_id = NEW.id AND f1_faser > NEW.faser_anz)
                      OR (f2_kabel_id = NEW.id AND f2_faser > NEW.faser_anz)) THEN
            RAISE EXCEPTION 'Fehler: Faser-Anzahl kann nicht auf % reduziert werden. Es sind bereits höhere Fasern auf Ports gepatcht.', NEW.faser_anz;
        END IF;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- 2. Den Trigger an die Tabelle 'kabel' hängen
CREATE TRIGGER trg_check_kabel_update
    BEFORE UPDATE
    ON kabel
    FOR EACH ROW
EXECUTE FUNCTION check_kabel_update_limits();