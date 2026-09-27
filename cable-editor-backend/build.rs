fn main() {
    // embed_migrations! reads the directory, cargo doesn't notice a new migration by itself
    println!("cargo:rerun-if-changed=migrations");
    cynic_codegen::register_schema("netbox")
        .from_sdl_file("./schema/schema.graphql")
        .expect("Cannot load graphql file")
        .as_default()
        .expect("cannot generate authenticated graphql schema for cynic");
}
