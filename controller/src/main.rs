#[macro_use]
extern crate log;

use migration::MigratorTrait;
use sea_orm::prelude::*;
use sea_orm::QueryOrder;
use sea_orm::QuerySelect;
use futures_util::StreamExt;

const TASK_QUEUE_NAME: &'static str = "spider_tasks";
const TASK_RESP_QUEUE_NAME: &'static str = "spider_tasks_resp";

#[derive(serde::Serialize)]
struct Task {
    id: uuid::Uuid,
    url: String
}


#[derive(serde::Deserialize)]
struct TaskResponse {
    id: uuid::Uuid,
    timestamp: chrono::DateTime<chrono::Utc>,
    success: bool,
    discovered_urls: Vec<String>
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    let db_url = std::env::var("DB_URL").expect("Environment variable DB_URL not set");
    let amqp_url = std::env::var("AMQP_URL").expect("Environment variable AMQP_URL not set");

    let db: std::sync::Arc<DatabaseConnection> = std::sync::Arc::new(
        sea_orm::Database::connect(db_url).await.expect("Unable to connect to database")
    );
    let amqp = lapin::Connection::connect(&amqp_url, lapin::ConnectionProperties::default())
        .await.expect("Unable to connect to RabbitMQ");
    migration::Migrator::up(db.as_ref(), None).await.expect("Unable to apply migrations");

    let task_channel = amqp.create_channel().await.expect("Unable to create RabbitMQ channel");
    task_channel.queue_declare(
        TASK_QUEUE_NAME,
        lapin::options::QueueDeclareOptions {
            durable: true,
            ..lapin::options::QueueDeclareOptions::default()
        },
        lapin::types::FieldTable::default(),
    ).await.expect("Unable to create task queue");

    let response_channel = amqp.create_channel().await.expect("Unable to create RabbitMQ channel");
    response_channel.basic_qos(50, lapin::options::BasicQosOptions::default())
        .await.expect("Unable to set channel QoS");
    response_channel.queue_declare(
        TASK_RESP_QUEUE_NAME,
        lapin::options::QueueDeclareOptions {
            durable: true,
            ..lapin::options::QueueDeclareOptions::default()
        },
        lapin::types::FieldTable::default(),
    ).await.expect("Unable to create task response queue");

    let resp_consumer = response_channel.basic_consume(
        TASK_RESP_QUEUE_NAME,
        "",
        lapin::options::BasicConsumeOptions::default(),
        lapin::types::FieldTable::default(),
    ).await.expect("Unable to create task queue consumer");

    let resp_db = db.clone();
    tokio::task::spawn(async move {
        handle_responses(resp_consumer, resp_db).await
    });

    loop {
        let urls = match get_urls_to_crawl(&db).await {
            Ok(urls) => urls,
            Err(e) => {
                error!("Failed to get URLs to crawl from database: {}", e);
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                continue;
            }
        };

        if urls.is_empty() {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            continue;
        }

        for url in urls {
            if let Err(e) = task_channel.basic_publish(
                "", TASK_QUEUE_NAME, lapin::options::BasicPublishOptions::default(),
                &serde_json::to_vec(&Task {
                    id: url.id,
                    url: url.url.clone(),
                }).unwrap(), lapin::BasicProperties::default()
                    .with_delivery_mode(2)
            ).await {
                warn!("Unable to publish task: {}", e);
                continue;
            }

            let mut url: entity::url::ActiveModel = url.into();
            url.pending_crawl = sea_orm::Set(true);
            if let Err(e) = url.update(db.as_ref()).await {
                warn!("Unable to update URL: {}", e);
                continue;
            }
        };
    }
}

async fn get_urls_to_crawl(db: &DatabaseConnection) -> Result<Vec<entity::url::Model>, DbErr> {
    let search_cutoff = chrono::Utc::now() - chrono::Duration::days(1);

    entity::url::Entity::find()
        .filter(
            sea_orm::Condition::any()
                .add(entity::url::Column::LastFetch.lte(search_cutoff))
                .add(entity::url::Column::LastFetch.is_null())
        )
        .filter(entity::url::Column::PendingCrawl.eq(false))
        .order_by_asc(entity::url::Column::LastSuccessfulFetch)
        .limit(1000)
        .all(db)
        .await
}

async fn handle_responses(mut consumer: lapin::Consumer, db: std::sync::Arc<DatabaseConnection>) {
    while let Some(delivery) = consumer.next().await {
        let delivery = delivery.expect("error in consumer");

        let db = db.clone();
        tokio::task::spawn(async move {
            let task_resp: TaskResponse = match serde_json::from_slice(&delivery.data) {
                Ok(t) => t,
                Err(e) => {
                    warn!("Invalid task response: {}", e);
                    delivery.reject(lapin::options::BasicRejectOptions {
                        requeue: false
                    }).await.expect("Unable to reject message");
                    return
                }
            };

            let mut url = entity::url::ActiveModel {
                pending_crawl: sea_orm::Set(false),
                last_fetch: sea_orm::Set(Some(task_resp.timestamp.naive_utc())),
                ..Default::default()
            };

            if task_resp.success {
                url.last_successful_fetch = sea_orm::Set(Some(task_resp.timestamp.naive_utc()));
            }

            if let Err(e) = entity::url::Entity::update_many()
                .set(url)
                .filter(entity::url::Column::Id.eq(task_resp.id))
                .exec(db.as_ref())
                .await {
                warn!("Failed to update database: {}", e);
                delivery.nack(lapin::options::BasicNackOptions::default())
                    .await.expect("Unable to reject message");
            }

            let discovery_db = db.clone();
            tokio::task::spawn(async move {
                handle_discovered_urls(task_resp, discovery_db).await;
            });

            delivery.ack(lapin::options::BasicAckOptions::default()).await.expect("Unable to ack message");
        });
    }
}

async fn handle_discovered_urls(task_resp: TaskResponse, db: std::sync::Arc<DatabaseConnection>) {
    for url in task_resp.discovered_urls {
        let url = match reqwest::Url::parse(&url) {
            Ok(u) => u,
            Err(e) => {
                warn!("Invalid URL {}: {}", url, e);
                continue
            }
        };

        let url_string = url.to_string();
        let url_count = match entity::url::Entity::find()
            .filter(entity::url::Column::Url.eq(&url_string))
            .count(db.as_ref()).await {
            Ok(c) => c,
            Err(e) => {
                warn!("Failed to search database: {}", e);
                continue
            }
        };
        if url_count == 0 {
            let new_url = entity::url::ActiveModel {
                id:  sea_orm::Set(uuid::Uuid::new_v4()),
                url: sea_orm::Set(url_string),
                pending_crawl: sea_orm::Set(false),
                last_fetch: sea_orm::Set(None),
                last_successful_fetch: sea_orm::Set(None),
                host: sea_orm::Set(url.host_str().map(|h| h.to_string()))
            };
            if let Err(e) = new_url.insert(db.as_ref()).await {
                warn!("Failed to insert URL: {}", e);
                continue
            }
        }
    }
}