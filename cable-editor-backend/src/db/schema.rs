diesel::table! {
    use postgis_diesel::sql_types::Geometry;
    use diesel::sql_types::{Integer, Text};
    schacht (id) {
        id -> Integer,
        geom -> Geometry,
        name -> Text,
        typ -> Integer,
    }
}

diesel::table! {
    use postgis_diesel::sql_types::Geometry;
    use diesel::sql_types::{Integer, Text};
    schacht_typ (id) {
        id -> Integer,
        name -> Text,
        icon -> Text,
    }
}
diesel::joinable!(schacht -> schacht_typ (typ));

diesel::allow_tables_to_appear_in_same_query!(schacht, schacht_typ);
