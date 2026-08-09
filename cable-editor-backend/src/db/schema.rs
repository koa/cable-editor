pub mod sql_types {
    #[derive(diesel::query_builder::QueryId, Clone, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "xml", schema = "pg_catalog"))]
    pub struct Xml;
    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "port_type_enum"))]
    pub struct PortTypeEnum;
    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "plan_status_enum"))]
    pub struct PlanStatusEnum;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "port_side_enum"))]
    pub struct PortSideEnum;
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
    panel (id) {
        id -> Int4,
        #[max_length = 20]
        name -> Nullable<Varchar>,
        schacht_id -> Int4,
        parent_panel -> Nullable<Int4>,
        parent_order -> Nullable<Int4>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::PortTypeEnum;

    panel_port (id) {
        id -> Int4,
        panel_id -> Int4,
        port_order -> Int4,
        #[max_length = 20]
        label -> Nullable<Varchar>,
        port_type -> PortTypeEnum,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::PlanStatusEnum;

    plan (id) {
        id -> Int4,
        #[max_length = 50]
        name -> Varchar,
        status -> PlanStatusEnum,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::PortSideEnum;

    port_usage (port_id, plan_id, side) {
        port_id -> Int4,
        plan_id -> Int4,
        side -> PortSideEnum,
        cable -> Nullable<Int4>,
        fiber -> Nullable<Int4>,
        bundle -> Nullable<Int4>,
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
diesel::joinable!(panel -> schacht (schacht_id));
diesel::joinable!(panel_port -> panel (panel_id));
diesel::joinable!(port_usage -> kabel (cable));
diesel::joinable!(port_usage -> panel_port (port_id));
diesel::joinable!(port_usage -> plan (plan_id));

diesel::allow_tables_to_appear_in_same_query!(
    kabel,
    kabel_trasse,
    panel,
    panel_port,
    plan,
    port_usage,
    schacht,
    schacht_typ,
    trasse,
    trassen_mit_endpunkten
);
