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
    use diesel::sql_types::{Integer, Text};
    schacht_typ (id) {
        id -> Integer,
        name -> Text,
        icon -> Text,
    }
}

diesel::table! {
    use diesel::sql_types::{Integer, Text};
    kabel(id){
        id -> Integer,
        name -> Text,
        buendel_anz -> Integer,
        faser_anz -> Integer
    }
}

diesel::table! {
    use diesel::sql_types::{Integer, Text};
    use postgis_diesel::sql_types::Geometry;
    trasse(id){
        id -> Integer,
        geom -> Geometry,
        description -> Text,
        schacht_a -> Integer,
        schacht_z -> Integer,
    }
}

//diesel::alias!(schacht as schacht_a);
//diesel::alias!(schacht as schacht_z);

diesel::joinable!(schacht -> schacht_typ (typ));

diesel::allow_tables_to_appear_in_same_query!(schacht, schacht_typ);
diesel::allow_tables_to_appear_in_same_query!(schacht, trasse,);
