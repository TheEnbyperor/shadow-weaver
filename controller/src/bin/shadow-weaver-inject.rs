use sea_orm::prelude::*;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    let args: Vec<String> = std::env::args().collect();

    let db_url = std::env::var("DB_URL").expect("Environment variable DB_URL not set");

    let db: DatabaseConnection = sea_orm::Database::connect(db_url).await.expect("Unable to connect to database");

    let url = reqwest::Url::parse(&args[1]).expect("Invalid url");

    let new_url = entity::url::ActiveModel {
        id:  sea_orm::Set(uuid::Uuid::new_v4()),
        url: sea_orm::Set(url.to_string()),
        pending_crawl: sea_orm::Set(false),
        last_fetch: sea_orm::Set(None),
        last_successful_fetch: sea_orm::Set(None),
        host: sea_orm::Set(url.host_str().map(|h| h.to_string())),
    };
    new_url.insert(&db).await.expect("Unable to insert");
}