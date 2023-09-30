use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.alter_table(
            sea_query::Table::alter()
                .table(super::m20230930_094203_create_url_table::URL::Table)
                .add_column(
                    ColumnDef::new(super::m20230930_094203_create_url_table::URL::PendingCrawl)
                        .boolean().not_null()
                )
                .to_owned()
        ).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.alter_table(
            sea_query::Table::alter()
                .table(super::m20230930_094203_create_url_table::URL::Table)
                .drop_column(super::m20230930_094203_create_url_table::URL::PendingCrawl)
                .to_owned()
        ).await
    }
}
