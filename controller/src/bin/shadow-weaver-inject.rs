use sea_orm::prelude::*;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    let args: Vec<String> = std::env::args().collect();

    let db_url = std::env::var("DB_URL").expect("Environment variable DB_URL not set");

    let db: DatabaseConnection = sea_orm::Database::connect(db_url).await.expect("Unable to connect to database");

    let new_url = entity::url::ActiveModel {
        id:  sea_orm::Set(uuid::Uuid::new_v4()),
        url: sea_orm::Set(args[1].clone()),
        pending_crawl: sea_orm::Set(false),
        last_fetch: sea_orm::Set(None),
        last_successful_fetch: sea_orm::Set(None),
    };
    new_url.insert(&db).await.expect("Unable to insert");
}