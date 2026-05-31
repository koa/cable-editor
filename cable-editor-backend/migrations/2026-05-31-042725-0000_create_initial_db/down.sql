-- This file should undo anything in `up.sql`
drop view trassen_mit_kabel_details;
drop view kabel_pfad;
drop view trassen_mit_endpunkten;
drop index sidx_trasse_geom;
drop table kabel_trasse;
drop table trasse;
drop index sidx_schacht_geom;
drop table schacht;
drop table schacht_typ;
drop table kabel;