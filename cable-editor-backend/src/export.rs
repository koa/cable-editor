use crate::db::entity::{schacht::Schacht, trasse::TrasseMitEndpunkten};
use diesel::HasQuery;
use diesel_async::{AsyncPgConnection, RunQueryDsl};

// Passe den Modellnamen und die Topic an das exakte Geolion-Modell 3343 an
const ILI_MODEL_NAME: &str = "LKMap_ZH_V1_0"; // Platzhalter für das echte Modell
const ILI_TOPIC_NAME: &str = "Leitungskataster";

pub async fn export_to_interlis2(
    conn: &mut AsyncPgConnection,
) -> Result<String, Box<dyn std::error::Error>> {
    // 1. Daten aus der DB abfragen
    let schaechte = Schacht::query().load(conn).await?;
    let trassen = TrasseMitEndpunkten::query().load(conn).await?;

    // 2. XML Header aufbauen
    let mut xtf = String::new();
    xtf.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xtf.push_str("<TRANSFER xmlns=\"http://www.interlis.ch/INTERLIS2.3\">\n");
    xtf.push_str("  <HEADERSECTION SENDER=\"cable-editor-backend\" VERSION=\"2.3\">\n");
    xtf.push_str("    <MODELS>\n");
    xtf.push_str(&format!(
        "      <MODEL NAME=\"{}\" VERSION=\"2026-01-01\" URI=\"https://geolion.zh.ch/\"/>\n",
        ILI_MODEL_NAME
    ));
    xtf.push_str("    </MODELS>\n");
    xtf.push_str("  </HEADERSECTION>\n");

    xtf.push_str("  <DATASECTION>\n");
    xtf.push_str(&format!(
        "    <{}.{} BID=\"b1\">\n",
        ILI_MODEL_NAME, ILI_TOPIC_NAME
    ));

    // 3. Schächte als Punkte (Punkte im INTERLIS Format <COORD><C1>x</C1><C2>y</C2></COORD>)
    for s in schaechte {
        if let Some(geom) = s.geom {
            xtf.push_str(&format!(
                "      <{}.{}.Schacht TID=\"schacht_{}\">\n",
                ILI_MODEL_NAME, ILI_TOPIC_NAME, s.id
            ));
            if let Some(name) = s.name {
                xtf.push_str(&format!("        <Name>{}</Name>\n", name));
            }
            xtf.push_str("        <Geometrie>\n");
            xtf.push_str(&format!(
                "          <COORD><C1>{:.3}</C1><C2>{:.3}</C2></COORD>\n",
                geom.x, geom.y
            ));
            xtf.push_str("        </Geometrie>\n");
            xtf.push_str(&format!(
                "      </{}.{}.Schacht>\n",
                ILI_MODEL_NAME, ILI_TOPIC_NAME
            ));
        }
    }

    // 4. Trassen als Linien (Linien sind <POLYLINE> aus mehreren <COORD>)
    for t in trassen {
        if let Some(linestring) = t.geom {
            xtf.push_str(&format!(
                "      <{}.{}.Trasse TID=\"trasse_{}\">\n",
                ILI_MODEL_NAME, ILI_TOPIC_NAME, t.id
            ));
            xtf.push_str("        <Geometrie>\n");
            xtf.push_str("          <POLYLINE>\n");

            // postgis_diesel speichert die Koordinaten in der inneren Geometrie
            for point in &linestring.points {
                xtf.push_str(&format!(
                    "            <COORD><C1>{:.3}</C1><C2>{:.3}</C2></COORD>\n",
                    point.x, point.y
                ));
            }

            xtf.push_str("          </POLYLINE>\n");
            xtf.push_str("        </Geometrie>\n");
            xtf.push_str(&format!(
                "      </{}.{}.Trasse>\n",
                ILI_MODEL_NAME, ILI_TOPIC_NAME
            ));
        }
    }

    xtf.push_str(&format!("    </{}.{}>\n", ILI_MODEL_NAME, ILI_TOPIC_NAME));
    xtf.push_str("  </DATASECTION>\n");
    xtf.push_str("</TRANSFER>\n");

    Ok(xtf)
}
