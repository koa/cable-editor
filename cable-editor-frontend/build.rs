use std::fs;

use anyhow::Result;
use cable_editor_backend::graphql::{
    anonymous::create_anonymous_schema, authenticated::create_authenticated_schema,
};

fn main() -> Result<()> {
    write_graphql_schema()?;
    write_anonymous_graphql_schema()?;
    Ok(())
}

fn write_graphql_schema() -> Result<()> {
    let schema = create_authenticated_schema();
    fs::write("graphql/authenticated_schema.graphql", schema.sdl())?;
    cynic_codegen::register_schema("authenticated")
        .from_sdl(schema.sdl().as_str())?
        .as_default()?;
    Ok(())
}
fn write_anonymous_graphql_schema() -> Result<()> {
    let schema = create_anonymous_schema();
    fs::write("graphql/anonymous_schema.graphql", schema.sdl())?;
    cynic_codegen::register_schema("anonymous")
        .from_sdl_file("graphql/anonymous_schema.graphql")?;
    Ok(())
}
