//! Every message the app words for errors of the backend (see docs/fehlermeldungen.md): the
//! reasons a request was refused (`UserError`) and the problems the Netbox sync reports. The
//! backend only sends their data.

use crate::graphql::authenticated::netbox_sync::SyncIssue;
use cable_editor_common::{
    ObjectKind, UserError,
    error::{NetboxStep, PlanPorts, PortUsageProblem},
};
use itertools::Itertools;

/// Why the backend refused a request.
pub fn user_error(error: &UserError) -> String {
    match error {
        UserError::NotLoggedIn => "Nicht angemeldet, bitte die Seite neu laden".into(),
        UserError::NotAllowed => "Keine Berechtigung für diese Änderung".into(),
        UserError::NotFound { kind, id } => not_found(*kind, *id),
        UserError::InvalidRequest => "Ungültige Anfrage (ein Fehler der App)".into(),

        UserError::NameMissing => "Der Name fehlt".into(),
        UserError::NameTooLong { max } => {
            format!("Der Name darf höchstens {max} Zeichen lang sein")
        }
        UserError::NameTaken { kind, name } => {
            format!("Der Name „{name}“ hat schon ein anderer {}", object(*kind))
        }
        UserError::DescriptionTooLong { max } => {
            format!("Die Beschreibung darf höchstens {max} Zeichen lang sein")
        }

        UserError::PositionNotConvertible => "Die Position konnte nicht umgerechnet werden".into(),
        UserError::PositionOutsideSwitzerland { e, n } => {
            format!("Die Position E {e:.2} / N {n:.2} liegt nicht in der Schweiz")
        }
        UserError::LineWithoutPoints => "Der Verlauf enthält keine Punkte".into(),
        UserError::LineNotConvertible => "Der Verlauf konnte nicht umgerechnet werden".into(),
        UserError::SchaechteAtSamePlace => {
            "Anfangs- und Endschacht liegen am selben Ort, die Richtung ist unbestimmt".into()
        }
        UserError::SchachtWithoutPosition { schacht } => {
            format!("Schacht {schacht} hat keine Position, der Verlauf kann nicht geprüft werden")
        }
        UserError::LineNeedsConfirmation {
            start_distance,
            end_distance,
        } => format!(
            "Der Verlauf endet {start_distance:.1} m bzw. {end_distance:.1} m von den Schächten \
             entfernt, bitte bestätigen"
        ),

        UserError::SchachtReferenced { panels, ducts } => {
            format!("Der Schacht hat noch {panels} Panels und {ducts} Trassen")
        }

        UserError::SameSchachtAtBothEnds => {
            "Anfangs- und Endschacht müssen verschieden sein".into()
        }
        UserError::DuctEndsFixed => {
            "Durch die Trasse führen Kabel, ihre Schächte können nicht geändert werden".into()
        }
        UserError::DuctHasCables { cables } => {
            format!("Durch die Trasse führen noch {cables} Kabel")
        }
        UserError::WidthOutOfRange { max } => {
            format!("Die Breite muss zwischen 0 und {max} mm liegen")
        }

        UserError::CableWithoutSegment => "Ein Kabel braucht mindestens ein Segment".into(),
        UserError::CableAttached { plans } => format!(
            "Das Kabel ist noch an Ports angeschlossen ({}), zuerst die Fasern lösen",
            plans
                .iter()
                .map(|PlanPorts { plan, ports }| format!("{plan}: {ports}"))
                .join(", ")
        ),
        UserError::DuctsNotConnected { duct, next } => {
            format!("Die Trassen {duct} und {next} sind nicht verbunden")
        }
        UserError::DuctNotAtSchacht { duct, schacht } => {
            format!("Die Trasse {duct} führt nicht zum Schacht {schacht}")
        }

        // A toast lists them below (`components::port_usage_issues`)
        UserError::PortUsagesBroken { usages } => match usages.len() {
            1 => "Die Änderung passt nicht zu einer Port-Belegung".into(),
            count => format!("Die Änderung passt nicht zu {count} Port-Belegungen"),
        },

        UserError::DefaultOwnerNotDeletable => {
            "Der Standard-Eigentümer kann nicht gelöscht werden, zuerst einen anderen als Standard \
             setzen"
                .into()
        }
        UserError::OwnerReferenced { schaechte, ducts } => {
            format!("Dem Eigentümer gehören noch {schaechte} Schächte und {ducts} Trassen")
        }
        UserError::LkNameTooLong { max } => {
            format!("Der Name in der Lieferung darf höchstens {max} Zeichen lang sein")
        }

        UserError::SchachtTypReferenced { schaechte } => {
            format!("Der Schachttyp hat noch {schaechte} Schächte")
        }
        UserError::DimensionOutOfRange { dimension, max } => {
            format!("Dimension {dimension} muss zwischen 0 und {max} mm liegen")
        }
        UserError::Dimension2WithoutDimension1 => {
            "Dimension 2 (das kleinere Mass) nur zusammen mit Dimension 1".into()
        }
        UserError::DimensionsSwapped => {
            "Dimension 1 ist das grössere, Dimension 2 das kleinere Mass".into()
        }
        UserError::IconTooLarge { max_bytes } => {
            format!("Das Icon ist grösser als {} KiB", max_bytes / 1024)
        }
        UserError::IconNotSvg => "Das Icon ist keine SVG-Datei".into(),

        UserError::BaselineUnchangeable => {
            "Der Ist-Zustand ändert sich nur durch das Umsetzen einer Planung".into()
        }

        UserError::LkmapNotConfigured => {
            "Die Lieferung an den Leitungskataster ist nicht konfiguriert (Abschnitt lkmap)".into()
        }
        UserError::LkmapIdTooLarge { kind, id } => format!(
            "{} {id}: die Id hat mehr als 7 Stellen und passt nicht in die OID der Lieferung",
            object(*kind)
        ),
        UserError::NothingToDeliver => {
            "Es gibt nichts zu liefern (keine Trasse mit Position an den Leitungskataster)".into()
        }

        UserError::NetboxFailed {
            step,
            object,
            detail,
        } => format!(
            "Netbox hat das {} {object} abgelehnt: {detail}",
            netbox_step(*step)
        ),
        UserError::NetboxWithoutId { step, object } => format!(
            "Netbox hat beim {} {object} keine Id geliefert",
            netbox_step(*step)
        ),
    }
}

/// Why a port usage doesn't fit its cable, as the end of a sentence.
pub fn port_usage_problem(problem: PortUsageProblem) -> &'static str {
    match problem {
        PortUsageProblem::CableNotEnding => "das Kabel endet nicht im Schacht",
        PortUsageProblem::FiberOutOfRange => "das Kabel hat diese Faser nicht",
        PortUsageProblem::FiberTwice => "die Faser liegt auch an einem anderen Port",
    }
}

/// An object that doesn't exist, whether the backend or a page found out.
pub fn not_found(kind: ObjectKind, id: i64) -> String {
    format!("{} {id} nicht gefunden", object(kind))
}

/// The kind of object in a sentence, e.g. "Schacht".
fn object(kind: ObjectKind) -> &'static str {
    match kind {
        ObjectKind::Schacht => "Schacht",
        ObjectKind::SchachtTyp => "Schachttyp",
        ObjectKind::Duct => "Trasse",
        ObjectKind::Cable => "Kabel",
        ObjectKind::Owner => "Eigentümer",
        ObjectKind::Plan => "Planung",
        ObjectKind::Panel => "Panel",
        ObjectKind::Port => "Port",
        ObjectKind::NetboxDevice => "Netbox-Gerät",
        ObjectKind::NetboxRearPort => "Netbox-RearPort",
        ObjectKind::LkmapDelivery => "Lieferung",
    }
}

/// What the Netbox sync was doing, after "das"/"beim".
fn netbox_step(step: NetboxStep) -> &'static str {
    match step {
        NetboxStep::DeleteCircuit => "Löschen des Circuits",
        NetboxStep::CreateCircuit => "Erstellen des Circuits",
        NetboxStep::UpdateCircuit => "Aktualisieren des Circuits",
        NetboxStep::CreateTermination => "Erstellen der Termination",
        NetboxStep::PatchCable => "Verbinden des Kabels",
    }
}

/// A problem the Netbox sync found; the ports of an asymmetric duplex are listed by the dialog.
pub fn sync_issue(issue: &SyncIssue) -> String {
    match issue {
        SyncIssue::MissingNetboxReference(error) => {
            format!("Port {} hat keine Netbox-Referenz", error.port.port_label())
        }
        SyncIssue::BlindEnd(error) => format!(
            "Die Verbindung ab Port {} endet nicht auf einem Stecker",
            error.port.port_label()
        ),
        SyncIssue::AsymmetricDuplex(error) => format!(
            "Verschiedene Netbox-Gegenstellen zu {}",
            error.start_netbox_port.display_name()
        ),
        SyncIssue::PortBlockedInNetbox(error) => format!(
            "Port {} ist in Netbox blockiert durch {}",
            error.port.port_label(),
            error.netbox_port.display_name()
        ),
        SyncIssue::NameCollision(error) => {
            format!("Der Circuit-Name {} ist schon vergeben", error.circuit_name)
        }
        SyncIssue::MissingNetboxMasterData(error) => {
            format!("In Netbox fehlen Stammdaten für {}", error.entity_type)
        }
        SyncIssue::RoutingLoop(error) => format!(
            "Die Verbindung ab Port {} läuft im Kreis",
            error.port.port_label()
        ),
        SyncIssue::InvalidTargetReference(error) => {
            format!("Ungültiges Ziel bei Port {}", error.port.port_label())
        }
        SyncIssue::Unknown => "Unbekanntes Problem (neuere Version des Servers?)".into(),
    }
}
