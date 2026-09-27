//! Limits and formats of the data, checked by the backend (the database would refuse the rest),
//! known to the frontend for its messages and inputs.

/// `schacht.name`, `schacht_typ.name` (`varchar(20)`)
pub const MAX_NAME: usize = 20;
/// `trasse.description` (`varchar(50)`)
pub const MAX_DESCRIPTION: usize = 50;
/// `Eigentuemer` in SIA405 (`TEXT*80`)
pub const MAX_LK_NAME: usize = 80;
/// `Breite` of an `LKLinie`, `Dimension1/2` of an `LKPunkt` (SIA405), millimetres
pub const MAX_MILLIMETRES: i32 = 4000;
/// An icon of a Schacht type: enough for a drawn symbol, a bigger file is likely a photo
pub const MAX_ICON_BYTES: usize = 64 * 1024;

/// A real (`CHE-`) or fictitious (`ZHE-`) UID like `CHE-123.456.789`, as the table eigentuemer
/// checks it.
pub fn is_uid(uid: &str) -> bool {
    let Some(digits) = uid
        .strip_prefix("CHE-")
        .or_else(|| uid.strip_prefix("ZHE-"))
    else {
        return false;
    };
    let groups: Vec<&str> = digits.split('.').collect();
    groups.len() == 3
        && groups
            .iter()
            .all(|g| g.len() == 3 && g.bytes().all(|b| b.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::is_uid;

    #[test]
    fn uids() {
        assert!(is_uid("CHE-123.456.789"));
        assert!(is_uid("ZHE-100.100.101"));
        assert!(!is_uid("CHE-123.456.78"));
        assert!(!is_uid("CHE-123456789"));
        assert!(!is_uid("DE-123.456.789"));
        assert!(!is_uid("CHE-12a.456.789"));
    }
}
