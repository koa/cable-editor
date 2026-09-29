pub mod sql_types {
    #[derive(diesel::query_builder::QueryId, Clone, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "xml", schema = "pg_catalog"))]
    pub struct Xml;
    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "port_type_enum"))]
    pub struct PortTypeEnum;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "port_side_enum"))]
    pub struct PortSideEnum;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "genauigkeit_enum"))]
    pub struct GenauigkeitEnum;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "lkmap_punkt_objektart_enum"))]
    pub struct LkmapPunktObjektartEnum;
}

diesel::table! {
    eigentuemer (id) {
        id -> Int4,
        name -> Text,
        #[max_length = 80]
        lk_name -> Nullable<Varchar>,
        standard -> Bool,
    }
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
    lk_lieferung (id) {
        id -> Int4,
        erstellt_am -> Timestamptz,
        erstellt_von -> Text,
        anzahl_schaechte -> Int4,
        anzahl_trassen -> Int4,
        #[max_length = 64]
        pruefsumme -> Bpchar,
        geliefert_am -> Nullable<Timestamptz>,
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
        netbox_device_id -> Nullable<Int4>
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
        netbox_port_id -> Nullable<Int4>,
    }
}

diesel::table! {
    plan (id) {
        id -> Int4,
        #[max_length = 50]
        name -> Varchar,
        netbox_active -> Bool,
    }
}

diesel::table! {
    netbox_sync_anstoss (id) {
        id -> Int8,
        txid -> Int8,
        erstellt -> Timestamptz,
    }
}

diesel::table! {
    netbox_sync (id) {
        id -> Bool,
        letzter_lauf -> Nullable<Timestamptz>,
        #[max_length = 10]
        ergebnis -> Nullable<Varchar>,
        fehler -> Nullable<Jsonb>,
        fehlversuche -> Int4,
        naechster_versuch -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    netbox_sync_issue (id) {
        id -> Int4,
        daten -> Jsonb,
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
    use super::sql_types::GenauigkeitEnum;

    schacht (id) {
            id -> Int4,
            geom -> Nullable<Geometry>,
            #[max_length = 20]
            name -> Nullable<Varchar>,
            typ -> Nullable<Int4>,
            eigentuemer_id -> Int4,
            lagebestimmung -> GenauigkeitEnum,
            geaendert_am -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::Int4;
    use diesel::sql_types::Nullable;
    use diesel::sql_types::Varchar;
    use super::sql_types::{LkmapPunktObjektartEnum, Xml};

    schacht_typ (id) {
        id -> Int4,
        #[max_length = 20]
        name -> Nullable<Varchar>,
        icon -> Xml,
        lkmap_objektart -> LkmapPunktObjektartEnum,
        dimension1_mm -> Nullable<Int4>,
        dimension2_mm -> Nullable<Int4>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use postgis_diesel::sql_types::Geometry;
    use super::sql_types::GenauigkeitEnum;

    trasse (id) {
            id -> Int4,
            geom -> Nullable<Geometry>,
            #[max_length = 50]
            description -> Nullable<Varchar>,
            schacht_a -> Int4,
            schacht_z -> Int4,
            eigentuemer_id -> Int4,
            leitungskataster -> Bool,
            lagebestimmung -> GenauigkeitEnum,
            breite_mm -> Nullable<Int4>,
            geaendert_am -> Timestamptz,
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
diesel::joinable!(schacht -> eigentuemer (eigentuemer_id));
diesel::joinable!(schacht -> schacht_typ (typ));
diesel::joinable!(trasse -> eigentuemer (eigentuemer_id));
diesel::joinable!(panel -> schacht (schacht_id));
diesel::joinable!(panel_port -> panel (panel_id));
diesel::joinable!(port_usage -> kabel (cable));
diesel::joinable!(port_usage -> panel_port (port_id));
diesel::joinable!(port_usage -> plan (plan_id));

diesel::allow_tables_to_appear_in_same_query!(
    eigentuemer,
    kabel,
    kabel_trasse,
    lk_lieferung,
    netbox_sync,
    netbox_sync_anstoss,
    netbox_sync_issue,
    panel,
    panel_port,
    plan,
    port_usage,
    schacht,
    schacht_typ,
    trasse,
    trassen_mit_endpunkten
);
