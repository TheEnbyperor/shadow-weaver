use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.create_index(sea_query::Index::create()
             .name("url-url")
             .table(super::m20230930_094203_create_url_table::URL::Table)
             .col(super::m20230930_094203_create_url_table::URL::Url)
             .to_owned()
        ).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_index(sea_query::Index::drop()
            .name("url-url")
            .to_owned()
        ).await
    }
}