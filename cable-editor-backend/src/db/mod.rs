pub mod entity;
pub mod schema;
use crate::error::BackendError;
use diesel::{Connection, PgConnection};
use diesel_async::{
    AsyncPgConnection,
    pooled_connection::{AsyncDieselConnectionManager, deadpool::Pool},
};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use log::info;

pub type DB = Pool<AsyncPgConnection>;
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

pub async fn connect() -> Result<DB, BackendError> {
    let db_url = std::env::var("DATABASE_URL").map_err(BackendError::CannotReadDatabaseUrl)?;
    let config = AsyncDieselConnectionManager::<AsyncPgConnection>::new(db_url);
    let pool = Pool::builder(config)
        .build()
        .map_err(BackendError::CannotConnectToDatabase)?;
    Ok(pool)
}

pub fn run_sync_migrations() {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let mut conn = PgConnection::establish(&database_url)
        .expect("Konnte keine Verbindung für Migrationen herstellen");
    let migrations = conn
        .run_pending_migrations(MIGRATIONS)
        .expect("Fehler beim Ausführen der Migrationen");
    info!("Migrations: {:?}", migrations);
}
