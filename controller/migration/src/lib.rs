pub use sea_orm_migration::prelude::*;

mod m20230930_094203_create_url_table;
mod m20230930_122301_url_pending;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20230930_094203_create_url_table::Migration),
            Box::new(m20230930_122301_url_pending::Migration),
        ]
    }
}
