use async_nats::jetstream::{self, stream::StorageType};
use std::time::Duration;

pub async fn ensure_autonomic_stream(
    client: &async_nats::Client,
) -> Result<jetstream::Context, String> {
    let js = jetstream::new(client.clone());
    js.get_or_create_stream(jetstream::stream::Config {
        name: agent_body_core::STREAM_NAME.to_string(),
        subjects: vec![agent_body_core::STREAM_SUBJECT_WILDCARD.to_string()],
        storage: StorageType::File,
        duplicate_window: agent_body_core::default_duplicate_window(),
        max_age: Duration::from_secs(7 * 24 * 3600),
        ..Default::default()
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(js)
}

pub async fn publish_dedup(
    js: &jetstream::Context,
    subject: &str,
    msg_id: &str,
    payload: &[u8],
) -> Result<(), String> {
    let mut headers = async_nats::HeaderMap::new();
    headers.insert("Nats-Msg-Id", msg_id);
    js.publish_with_headers(subject.to_string(), headers, payload.to_vec().into())
        .await
        .map_err(|e| e.to_string())?
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}
