use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "url")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: uuid::Uuid,
    pub url: String,
    pub last_fetch: Option<chrono::NaiveDateTime>,
    pub last_successful_fetch: Option<chrono::NaiveDateTime>,
    pub pending_crawl: bool,
    pub host: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
}

impl ActiveModelBehavior for ActiveModel {}