#[macro_use]
extern crate log;

use std::str::FromStr;
use futures_util::StreamExt;

const SOCKS_ADDR: &'static str = "tor";
const SOCKS_PORT: u16 = 9050;
const TASK_QUEUE_NAME: &'static str = "spider_tasks";
const TASK_RESP_QUEUE_NAME: &'static str = "spider_tasks_resp";

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    let amqp_url = std::env::var("AMQP_URL").expect("Environment variable AMQP_URL not set");
    let amqp = lapin::Connection::connect(&amqp_url, lapin::ConnectionProperties::default())
        .await.expect("Unable to connect to RabbitMQ");

    let task_channel = amqp.create_channel().await.expect("Unable to create RabbitMQ channel");
    task_channel.basic_qos(5, lapin::options::BasicQosOptions::default())
        .await.expect("Unable to set channel QoS");
    task_channel.queue_declare(
        TASK_QUEUE_NAME,
        lapin::options::QueueDeclareOptions {
            durable: true,
            ..lapin::options::QueueDeclareOptions::default()
        },
        lapin::types::FieldTable::default(),
    ).await.expect("Unable to create task queue");

    let mut task_consumer = task_channel.basic_consume(
        TASK_QUEUE_NAME,
        "",
        lapin::options::BasicConsumeOptions::default(),
        lapin::types::FieldTable::default(),
    ).await.expect("Unable to create task queue consumer");

    info!("Waiting for SOCKS proxy to become available");
    while tokio::net::TcpStream::connect((SOCKS_ADDR, SOCKS_PORT)).await.is_err() {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }

    info!("Starting spider");

    let client = reqwest::Client::builder()
        .user_agent(format!("ShadowWeaver {} +https://magicalcodewit.ch", env!("CARGO_PKG_VERSION")))
        .proxy(reqwest::Proxy::all(format!("socks5h://{}:{}", SOCKS_ADDR, SOCKS_PORT)).expect("Unable to setup proxy"))
        .gzip(true)
        .brotli(true)
        .deflate(true)
        .timeout(std::time::Duration::from_secs(300))
        .connect_timeout(std::time::Duration::from_secs(300))
        .http1_title_case_headers()
        .http1_ignore_invalid_headers_in_responses(true)
        .no_trust_dns()
        .build().expect("Unable to setup HTTP client");

    let task_channel = std::sync::Arc::new(task_channel);
    let client = std::sync::Arc::new(client);
    while let Some(delivery) = task_consumer.next().await {
        let delivery = delivery.expect("error in consumer");
        tokio::task::spawn(handle_delivery(delivery, task_channel.clone(), client.clone()));
    }
}

#[derive(serde::Deserialize)]
struct Task {
    id: uuid::Uuid,
    url: String
}


#[derive(serde::Serialize)]
struct TaskResponse {
    id: uuid::Uuid,
    timestamp: chrono::DateTime<chrono::Utc>,
    success: bool,
    discovered_urls: Vec<String>
}

async fn handle_delivery(
    delivery: lapin::message::Delivery, channel: std::sync::Arc<lapin::Channel>, client: std::sync::Arc<reqwest::Client>
) {
    let task: Task = match serde_json::from_slice(&delivery.data) {
        Ok(t) => t,
        Err(e) => {
            warn!("Invalid task: {}", e);
            delivery.reject(lapin::options::BasicRejectOptions {
                requeue: false
            }).await.expect("Unable to reject message");
            return
        }
    };

    let send_resp = |resp: TaskResponse| async move {
        channel.basic_publish(
            "", TASK_RESP_QUEUE_NAME, lapin::options::BasicPublishOptions::default(),
            &serde_json::to_vec(&resp).unwrap(), lapin::BasicProperties::default()
                .with_delivery_mode(2)
        ).await.expect("Unable to publish response");

        delivery.ack(lapin::options::BasicAckOptions::default())
            .await.expect("Unable to ack message");
    };

    let url = match reqwest::Url::parse(&task.url) {
        Ok(u) => u,
        Err(e) => {
            warn!("Invalid URL {}: {}", task.url, e);

            let resp = TaskResponse {
                id: task.id,
                timestamp: chrono::Utc::now(),
                success: false,
                discovered_urls: vec![]
            };
            send_resp(resp).await;
            return
        }
    };

    info!("Fetching {}", url);
    let links = match fetch_links(&client, url.clone()).await {
        Ok(l) => l,
        Err(e) => {
            warn!("Unable to fetch {}: {:?}", url, e);

            let resp = TaskResponse {
                id: task.id,
                timestamp: chrono::Utc::now(),
                success: false,
                discovered_urls: vec![]
            };
            send_resp(resp).await;
            return
        }
    };

    info!("Discovered {} links on {}", links.len(), url);
    let resp = TaskResponse {
        id: task.id,
        timestamp: chrono::Utc::now(),
        success: true,
        discovered_urls: links,
    };
    send_resp(resp).await;
}

#[derive(Debug)]
enum SpiderError {
    ReqwestError(reqwest::Error),
    ReqwestHeaderError(reqwest::header::ToStrError),
    MimieError(mime::FromStrError),
    ResponseCode(reqwest::StatusCode),
    IoError(std::io::Error),
}

impl From<reqwest::Error> for SpiderError {
    fn from(value: reqwest::Error) -> Self {
        Self::ReqwestError(value)
    }
}

impl From<reqwest::header::ToStrError> for SpiderError {
    fn from(value: reqwest::header::ToStrError) -> Self {
        Self::ReqwestHeaderError(value)
    }
}

impl From<mime::FromStrError> for SpiderError {
    fn from(value: mime::FromStrError) -> Self {
        Self::MimieError(value)
    }
}

impl From<std::io::Error> for SpiderError {
    fn from(value: std::io::Error) -> Self {
        Self::IoError(value)
    }
}

type SpiderResult<T> = Result<T, SpiderError>;

async fn fetch_links<U: reqwest::IntoUrl>(client: &reqwest::Client, url: U) -> SpiderResult<Vec<String>> {
    let url = url.into_url()?;
    let resp = client.get(url.clone()).send().await?;
    if !resp.status().is_success() {
        return Err(SpiderError::ResponseCode(resp.status()));
    }

    let content_type = match resp.headers().get(reqwest::header::CONTENT_TYPE) {
        Some(v) => v,
        None => return Ok(vec![]) // No idea how to extract links from an unknown content type
    };
    let content_type = mime::Mime::from_str(content_type.to_str()?)?;

    match (content_type.type_(), content_type.subtype()) {
        (mime::TEXT, mime::HTML) => {
            let body = std::io::Cursor::new(resp.bytes().await?);
            let document = select::document::Document::from_read(body)?;

            let links = document.find(select::predicate::Name("a"))
                .filter_map(|n| n.attr("href"))
                .filter_map(|u| match reqwest::Url::parse(u) {
                    Ok(u) => Some(u),
                    Err(url::ParseError::RelativeUrlWithoutBase) => url.join(u).ok(),
                    o => None
                })
                .filter(|u| matches!(u.scheme(), "http" | "https"))
                .filter(|u| u.host_str().unwrap().ends_with(".onion"))
                .map(|u| u.as_str().to_string())
                .collect::<Vec<_>>();

            Ok(links)
        }
        (t, s) => {
            warn!("Unknown Content-Type {t}/{s}");
            Ok(vec![])
        }
    }
}
