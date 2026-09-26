drop view trassen_mit_kabel_details;
alter table trasse add column eigenleistung boolean default true not null;

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
