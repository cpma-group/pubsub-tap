use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
    routing::get,
    Json, Router,
};
use base64::prelude::*;
use futures::stream::Stream;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, collections::HashSet, convert::Infallible, env, sync::Arc, time::Duration};
use tokio::sync::broadcast;
use tokio_stream::{wrappers::BroadcastStream, StreamExt};
use tower_http::services::{ServeDir, ServeFile};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
struct TapMessage {
    id: String,
    topic: String,
    project: String,
    data: serde_json::Value,
    attributes: HashMap<String, String>,
    publish_time: String,
    tapped_at: String,
}

#[derive(Deserialize)]
struct ListTopicsResponse {
    topics: Option<Vec<TopicResource>>,
}

#[derive(Deserialize)]
struct TopicResource {
    name: String,
}

#[derive(Deserialize)]
struct PullResponse {
    #[serde(rename = "receivedMessages", default)]
    received_messages: Vec<ReceivedMessage>,
}

#[derive(Deserialize)]
struct ReceivedMessage {
    #[serde(rename = "ackId")]
    ack_id: String,
    message: PubsubMessageResource,
}

#[derive(Deserialize)]
struct PubsubMessageResource {
    #[serde(rename = "messageId", default)]
    message_id: String,
    #[serde(default)]
    data: Option<String>,
    #[serde(default)]
    attributes: Option<HashMap<String, String>>,
    #[serde(rename = "publishTime", default)]
    publish_time: Option<String>,
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

struct AppState {
    tx: broadcast::Sender<TapMessage>,
    emulator_host: String,
    project_ids: Vec<String>,
    client: Client,
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let emulator_host =
        env::var("PUBSUB_EMULATOR_HOST").unwrap_or_else(|_| "localhost:8085".into());
    let project_ids: Vec<String> = env::var("GCP_PROJECT_IDS")
        .unwrap_or_else(|_| "my-project".into())
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let port: u16 = env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(4000);
    let static_dir = env::var("STATIC_DIR").unwrap_or_else(|_| "./static".into());

    let (tx, _) = broadcast::channel::<TapMessage>(2048);

    let state = Arc::new(AppState {
        tx,
        emulator_host,
        project_ids,
        client: Client::new(),
    });

    // Background: poll emulator for messages
    tokio::spawn(monitor_loop(Arc::clone(&state)));

    let api = Router::new()
        .route("/api/events", get(sse_handler))
        .route("/api/topics", get(topics_handler))
        .route("/healthz", get(health_handler))
        .with_state(Arc::clone(&state));

    let app = api.fallback_service(
        ServeDir::new(&static_dir)
            .not_found_service(ServeFile::new(format!("{}/index.html", static_dir))),
    );

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .expect("failed to bind");
    tracing::info!("pubsub-tap listening on 0.0.0.0:{}", port);
    tracing::info!("monitoring projects: {:?}", state.project_ids);

    axum::serve(listener, app).await.expect("server error");
}

// ---------------------------------------------------------------------------
// SSE handler — streams tapped messages to the browser
// ---------------------------------------------------------------------------

async fn sse_handler(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|res| {
        res.ok().and_then(|msg| {
            Event::default()
                .event("message")
                .json_data(&msg)
                .ok()
                .map(Ok)
        })
    });
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

// ---------------------------------------------------------------------------
// Health check
// ---------------------------------------------------------------------------

async fn health_handler() -> &'static str {
    "ok"
}

// ---------------------------------------------------------------------------
// Topics handler — returns discovered topic names
// ---------------------------------------------------------------------------

async fn topics_handler(State(state): State<Arc<AppState>>) -> Json<Vec<String>> {
    let mut topics = Vec::new();
    for project in &state.project_ids {
        if let Ok(list) = list_topics(&state.client, &state.emulator_host, project).await {
            for t in list {
                if let Some(short) = t.name.rsplit('/').next() {
                    topics.push(short.to_string());
                }
            }
        }
    }
    Json(topics)
}

// ---------------------------------------------------------------------------
// Monitor loop — discovers topics, taps messages, broadcasts via channel
// ---------------------------------------------------------------------------

async fn monitor_loop(state: Arc<AppState>) {
    let mut known_subs: HashSet<String> = HashSet::new();

    loop {
        for project in &state.project_ids {
            let topics = match list_topics(&state.client, &state.emulator_host, project).await {
                Ok(t) => t,
                Err(_) => continue,
            };

            for topic in &topics {
                let short_topic = topic.name.rsplit('/').next().unwrap_or(&topic.name);
                let sub_name = format!("tap-{}", short_topic);
                let sub_key = format!("{}/{}", project, sub_name);

                // Ensure tap subscription exists
                if known_subs.insert(sub_key.clone()) {
                    let url = format!(
                        "http://{}/v1/projects/{}/subscriptions/{}",
                        state.emulator_host, project, sub_name
                    );
                    let body = serde_json::json!({
                        "topic": topic.name,
                        "ackDeadlineSeconds": 600
                    });
                    match state.client.put(&url).json(&body).send().await {
                        Ok(resp) if resp.status().is_success() => {
                            tracing::info!("tap subscription created: {}", sub_key);
                        }
                        Ok(resp) if resp.status().as_u16() == 409 => {
                            tracing::debug!("tap subscription exists: {}", sub_key);
                        }
                        Ok(resp) => {
                            let status = resp.status();
                            let body = resp.text().await.unwrap_or_default();
                            tracing::warn!("failed to create sub {} ({}): {}", sub_key, status, body);
                            known_subs.remove(&sub_key);
                            continue;
                        }
                        Err(e) => {
                            tracing::warn!("failed to create sub {}: {}", sub_key, e);
                            known_subs.remove(&sub_key);
                            continue;
                        }
                    }
                }

                // Pull messages from tap subscription
                let pull_url = format!(
                    "http://{}/v1/projects/{}/subscriptions/{}:pull",
                    state.emulator_host, project, sub_name
                );
                let pull_body = serde_json::json!({ "maxMessages": 100, "returnImmediately": true });

                let resp = match state.client.post(&pull_url).json(&pull_body).send().await {
                    Ok(r) => r,
                    Err(_) => continue,
                };
                let pull: PullResponse = match resp.json().await {
                    Ok(p) => p,
                    Err(_) => continue,
                };

                if pull.received_messages.is_empty() {
                    continue;
                }

                let mut ack_ids = Vec::new();
                for rm in &pull.received_messages {
                    ack_ids.push(rm.ack_id.clone());

                    let raw = rm.message.data.as_deref().unwrap_or("");
                    let decoded = BASE64_STANDARD
                        .decode(raw)
                        .ok()
                        .and_then(|b| String::from_utf8(b).ok())
                        .unwrap_or_else(|| raw.to_string());

                    let data: serde_json::Value = serde_json::from_str(&decoded)
                        .unwrap_or_else(|_| serde_json::Value::String(decoded));

                    let tap_msg = TapMessage {
                        id: rm.message.message_id.clone(),
                        topic: short_topic.to_string(),
                        project: project.clone(),
                        data,
                        attributes: rm.message.attributes.clone().unwrap_or_default(),
                        publish_time: rm.message.publish_time.clone().unwrap_or_default(),
                        tapped_at: chrono::Utc::now()
                            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                    };

                    let _ = state.tx.send(tap_msg);
                }

                // Acknowledge so the tap subscription doesn't fill up
                let ack_url = format!(
                    "http://{}/v1/projects/{}/subscriptions/{}:acknowledge",
                    state.emulator_host, project, sub_name
                );
                let _ = state
                    .client
                    .post(&ack_url)
                    .json(&serde_json::json!({ "ackIds": ack_ids }))
                    .send()
                    .await;
            }
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

async fn list_topics(
    client: &Client,
    host: &str,
    project: &str,
) -> Result<Vec<TopicResource>, reqwest::Error> {
    let url = format!("http://{}/v1/projects/{}/topics", host, project);
    let resp = client.get(&url).send().await?;
    let list: ListTopicsResponse = resp.json().await?;
    Ok(list.topics.unwrap_or_default())
}
