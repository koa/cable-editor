create table kabel
(
    id          serial
        constraint kabel_pk
            primary key,
    name        varchar(20) not null,
    buendel_anz integer     not null,
    faser_anz   integer     not null
);

create table schacht_typ
(
    id   integer generated always as identity (minvalue 0)
        constraint schacht_typ_pk
            primary key,
    name varchar(20),
    icon xml not null
);

create table schacht
(
    id   serial
        primary key,
    geom geometry(Point, 2056),
    name varchar(20),
    typ  integer
        constraint schacht_schacht_typ_id_fk
            references schacht_typ
            on update restrict on delete restrict
);

create index sidx_schacht_geom
    on schacht using gist (geom);

create table trasse
(
    id            serial primary key,
    geom          geometry(LineString, 2056),
    description   varchar(50),
    schacht_a     integer              not null
        constraint trasse___fk_a
            references schacht,
    schacht_z     integer              not null
        constraint trasse___fk_z
            references schacht,
    eigenleistung boolean default true not null
);

create table kabel_trasse
(
    kabel   integer not null
        constraint kabel_trasse_kabel_id_fk
            references kabel,
    trasse  integer not null
        constraint kabel_trasse_trasse_id_fk
            references trasse,
    sequenz integer not null,
    constraint kabel_trasse_pk
        primary key (kabel, sequenz)
);

create index sidx_trasse_geom
    on trasse using gist (geom);


create view trassen_mit_endpunkten(id, sa_id, sa_name, sz_id, sz_name, geom) as
SELECT t.id,
       sa.id                                                                      AS sa_id,
       sa.name                                                                    AS sa_name,
       sz.id                                                                      AS sz_id,
       sz.name                                                                    AS sz_name,
       st_makeline(ARRAY [sa.geom, t.geom, sz.geom]) ::geometry(LineString, 2056) AS geom
FROM trasse t
         JOIN schacht sa ON t.schacht_a = sa.id
         JOIN schacht sz ON t.schacht_z = sz.id;

create view kabel_pfad(id, name, buendel_anz, faser_anz, geom) as
SELECT k.id,
       k.name,
       k.buendel_anz,
       k.faser_anz,
       st_linemerge(st_collect(t_1.geom ORDER BY kt_1.sequenz)) ::geometry(LineString, 2056) AS geom
FROM kabel k
         JOIN kabel_trasse kt_1 ON k.id = kt_1.kabel
         JOIN trassen_mit_endpunkten t_1 ON kt_1.trasse = t_1.id
GROUP BY k.id, k.name;


create view trassen_mit_kabel_details(id, kabel_details, anzahl_kabel, geom, eigenleistung) as
WITH kabel_laengen AS (SELECT k.id,
                              k.name,
                              k.buendel_anz,
                              k.faser_anz,
                              round(st_length(k.geom)) AS gesamt_laenge,
                              k.geom
                       FROM kabel_pfad k)
SELECT t.id,
       string_agg(((((((kl.name::text || ' '::text) || kl.buendel_anz) || 'x'::text) || kl.faser_anz) || ' ('::text) ||
                   kl.gesamt_laenge) || 'm)'::text, ', '::text ORDER BY kl.name) AS kabel_details,
       count(kl.id)                                                              AS anzahl_kabel,
       tme.geom,
       t.eigenleistung
FROM trasse t
         LEFT JOIN kabel_trasse kt ON t.id = kt.trasse
         LEFT JOIN kabel_laengen kl ON kt.kabel = kl.id
         LEFT JOIN trassen_mit_endpunkten tme ON t.id = tme.id
GROUP BY t.id, tme.geom, t.eigenleistung;