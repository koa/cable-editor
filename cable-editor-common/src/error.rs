//! Why a request was refused. The backend returns a `UserError` as GraphQL error with the
//! variant in `extensions.userError` (`{"code": "OwnerNameTaken", …}`); the frontend words it.
//! A variant holds what its message needs, never a text.

use serde::{Deserialize, Serialize};

/// The kind of an object that wasn't found or is named in a message.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    Schacht,
    SchachtTyp,
    Duct,
    Cable,
    Owner,
    Plan,
    Panel,
    Port,
    NetboxDevice,
    NetboxRearPort,
    LkmapDelivery,
}

/// A plan and how many ports in it a cable is attached to.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlanPorts {
    pub plan: Box<str>,
    pub ports: i64,
}

/// What the Netbox sync was doing when Netbox refused.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetboxStep {
    DeleteCircuit,
    CreateCircuit,
    UpdateCircuit,
    CreateTermination,
    PatchCable,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "code", rename_all_fields = "camelCase")]
pub enum UserError {
    /// The request came without a valid login
    NotLoggedIn,
    /// The user's role doesn't allow the change
    NotAllowed,
    NotFound {
        kind: ObjectKind,
        id: i64,
    },
    /// A request the frontend shouldn't send, e.g. a change of a port without id
    InvalidRequest,

    // Names and descriptions
    NameMissing,
    NameTooLong {
        max: usize,
    },
    NameTaken {
        kind: ObjectKind,
        name: Box<str>,
    },
    DescriptionTooLong {
        max: usize,
    },

    // Positions and courses (LV95)
    PositionNotConvertible,
    PositionOutsideSwitzerland {
        e: f64,
        n: f64,
    },
    LineWithoutPoints,
    LineNotConvertible,
    /// The Schächte of a duct lie at the same place, the direction of its course is unknown
    SchaechteAtSamePlace,
    SchachtWithoutPosition {
        schacht: Box<str>,
    },
    /// An end of the course lies far from its Schacht and wasn't confirmed
    LineNeedsConfirmation {
        start_distance: f64,
        end_distance: f64,
    },

    // Schächte
    SchachtReferenced {
        panels: i64,
        ducts: i64,
    },

    // Ducts
    SameSchachtAtBothEnds,
    /// Cables run through the duct, its Schächte can't change
    DuctEndsFixed,
    DuctHasCables {
        cables: i64,
    },
    WidthOutOfRange {
        max: i32,
    },

    // Cables
    CableWithoutSegment,
    CableAttached {
        plans: Box<[PlanPorts]>,
    },
    DuctsNotConnected {
        duct: i32,
        next: i32,
    },
    DuctNotAtSchacht {
        duct: i32,
        schacht: i32,
    },
    /// The path of a cable doesn't end at the Schacht it was asked from
    InvalidCableEnd {
        schacht: i32,
        cable: i32,
    },

    // Owners
    DefaultOwnerNotDeletable,
    OwnerReferenced {
        schaechte: i64,
        ducts: i64,
    },
    OwnerDelivered {
        deliveries: i64,
    },
    LkNameTooLong {
        max: usize,
    },
    InvalidUid {
        uid: Box<str>,
    },
    UidTaken {
        uid: Box<str>,
        owner: Box<str>,
    },

    // Schacht types
    SchachtTypReferenced {
        schaechte: i64,
    },
    /// Dimension 1 or 2 outside of 0..=max
    DimensionOutOfRange {
        dimension: u8,
        max: i32,
    },
    Dimension2WithoutDimension1,
    /// Dimension 2 (the smaller) larger than dimension 1
    DimensionsSwapped,
    IconTooLarge {
        max_bytes: usize,
    },
    IconNotSvg,

    // Plans
    /// The baseline changes only by implementing a plan
    BaselineUnchangeable,

    // Delivery to the Leitungskataster
    /// The section `lkmap` of the configuration is missing
    LkmapNotConfigured,
    /// The owner has no UID, so nothing of it can be delivered
    OwnerWithoutUid {
        owner: Box<str>,
    },
    /// An id doesn't fit into the 7 digits of an OID
    LkmapIdTooLarge {
        kind: ObjectKind,
        id: i32,
    },
    /// The owner has no delivered duct and no Schacht where one ends
    NothingToDeliver {
        owner: Box<str>,
    },

    // Netbox sync
    /// Netbox refused a step; `detail` is Netbox's answer, for support
    NetboxFailed {
        step: NetboxStep,
        object: Box<str>,
        detail: Box<str>,
    },
    /// Netbox answered without the id of what it created
    NetboxWithoutId {
        step: NetboxStep,
        object: Box<str>,
    },
}

/// Where an unexpected error of the backend (not a `UserError`) came from, in
/// `extensions.origin` of its GraphQL error: the frontend shows it for those who can tell what
/// it means, `id` finds it in the server's log.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ErrorOrigin {
    /// The crate whose error it is, e.g. `diesel`
    pub library: Box<str>,
    /// `file:line` of the code that got it
    pub location: Box<str>,
    pub id: Box<str>,
}

#[cfg(feature = "async-graphql")]
impl From<UserError> for async_graphql::Error {
    /// The message is only the code (for debugging), the variant goes to
    /// `extensions.userError`.
    fn from(error: UserError) -> Self {
        let value = serde_json::to_value(&error).unwrap_or_default();
        let code = value["code"].as_str().unwrap_or("UserError").to_string();
        use async_graphql::ErrorExtensions;
        let error = async_graphql::Error::new(code);
        match async_graphql::Value::from_json(value) {
            Ok(value) => error.extend_with(|_, extensions| extensions.set("userError", value)),
            Err(_) => error,
        }
    }
}

#[cfg(all(test, feature = "async-graphql"))]
mod tests {
    use super::*;

    #[test]
    fn as_graphql_error() {
        let error: async_graphql::Error = UserError::NameTaken {
            kind: ObjectKind::Owner,
            name: "X".into(),
        }
        .into();
        assert_eq!(error.message, "NameTaken");
        let extensions = serde_json::to_value(error.extensions).unwrap_or_default();
        assert_eq!(
            extensions["userError"],
            serde_json::json!({"code": "NameTaken", "kind": "Owner", "name": "X"})
        );
    }
}
