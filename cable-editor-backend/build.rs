fn main() {
    cynic_codegen::register_schema("netbox")
        .from_sdl_file("./schema/schema.graphql")
        .expect("Cannot load graphql file")
        .as_default()
        .expect("cannot generate authenticated graphql schema for cynic");
}
