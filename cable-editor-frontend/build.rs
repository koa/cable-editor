use anyhow::Result;
use cable_editor_backend::graphql::{
    anonymous::create_anonymous_schema, authenticated::create_authenticated_schema,
};
use std::fs;

fn main() -> Result<()> {
    if !fs::exists("graphql").expect("Cannot test for graphql folder") {
        fs::create_dir("graphql").expect("Cannot create graphql folder");
    }
    write_graphql_schema();
    write_anonymous_graphql_schema();
    Ok(())
}

fn write_graphql_schema() {
    let schema = create_authenticated_schema();
    fs::write("graphql/authenticated_schema.graphql", schema.sdl())
        .expect("Cannot write authenticated graphql schema");
    cynic_codegen::register_schema("authenticated")
        .from_sdl(schema.sdl().as_str())
        .expect("Cannot load authenticated graphql file")
        .as_default()
        .expect("cannot generate authenticated graphql schema for cynic");
}
fn write_anonymous_graphql_schema() {
    let schema = create_anonymous_schema();
    fs::write("graphql/anonymous_schema.graphql", schema.sdl())
        .expect("Cannot write anonymous graphql schema");
    cynic_codegen::register_schema("anonymous")
        .from_sdl_file("graphql/anonymous_schema.graphql")
        .expect("Cannot load anonymous graphql file");
}
