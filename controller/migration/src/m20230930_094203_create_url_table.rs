use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(URL::Table)
                    .col(
                        ColumnDef::new(URL::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(URL::Url).string().not_null())
                    .col(ColumnDef::new(URL::LastFetch).timestamp().null())
                    .col(ColumnDef::new(URL::LastSuccessfulFetch).timestamp().null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(URL::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum URL {
    Table,
    Id,
    Url,
    LastFetch,
    LastSuccessfulFetch,
    PendingCrawl,
    Host,
}
