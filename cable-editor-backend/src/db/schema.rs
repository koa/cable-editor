pub mod sql_types {
    #[derive(diesel::query_builder::QueryId, Clone, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "xml", schema = "pg_catalog"))]
    pub struct Xml;
}

diesel::table! {
    kabel (id) {
        id -> Int4,
        #[max_length = 20]
        name -> Varchar,
        buendel_anz -> Int4,
        faser_anz -> Int4,
    }
}

diesel::table! {
    kabel_trasse (kabel, sequenz) {
        kabel -> Int4,
        trasse -> Int4,
        sequenz -> Int4,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use postgis_diesel::sql_types::Geometry;

    schacht (id) {
            id -> Int4,
            geom -> Nullable<Geometry>,
            #[max_length = 20]
            name -> Nullable<Varchar>,
            typ -> Nullable<Int4>,
    }
}

diesel::table! {
    use diesel::sql_types::Int4;
    use diesel::sql_types::Nullable;
    use diesel::sql_types::Varchar;
    use super::sql_types::Xml;

    schacht_typ (id) {
        id -> Int4,
        #[max_length = 20]
        name -> Nullable<Varchar>,
        icon -> Xml,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use postgis_diesel::sql_types::Geometry;

    trasse (id) {
            id -> Int4,
            geom -> Nullable<Geometry>,
            #[max_length = 50]
            description -> Nullable<Varchar>,
            schacht_a -> Int4,
            schacht_z -> Int4,
            eigenleistung -> Bool,
    }
}

diesel::table! {
    use diesel::sql_types::Int4;
    use diesel::sql_types::Nullable;
    use diesel::sql_types::Varchar;
    use postgis_diesel::sql_types::Geometry;
    trassen_mit_endpunkten(id){
        id -> Int4,
        geom -> Nullable<Geometry>,
        sa_id -> Int4,
        #[max_length = 20]
        sa_name -> Nullable<Varchar>,
        sz_id -> Int4,
        #[max_length = 20]
        sz_name -> Nullable<Varchar>,

    }
}

diesel::joinable!(kabel_trasse -> kabel (kabel));
diesel::joinable!(kabel_trasse -> trasse (trasse));
diesel::joinable!(kabel_trasse -> trassen_mit_endpunkten (trasse));
diesel::joinable!(schacht -> schacht_typ (typ));

diesel::allow_tables_to_appear_in_same_query!(
    kabel,
    kabel_trasse,
    schacht,
    schacht_typ,
    trasse,
    trassen_mit_endpunkten
);
