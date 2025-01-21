use presage::libsignal_service::content::Reaction;
use presage::libsignal_service::prelude::Uuid;
use presage::libsignal_service::proto::data_message::Quote;
use presage::{
    libsignal_service::content::{ContentBody, DataMessage},
    manager::Registered,
    store::{Store, Thread},
    Manager,
};
use tracing::warn;

pub async fn format_data_message<S: Store>(
    thread: &Thread,
    data_message: &DataMessage,
    manager: &Manager<S, Registered>,
) -> Option<String> {
    match data_message {
        DataMessage {
            quote:
                Some(Quote {
                    text: Some(quoted_text),
                    ..
                }),
            body: Some(body),
            ..
        } => Some(format!("Answer to message \"{quoted_text}\": {body}")),
        DataMessage {
            reaction:
                Some(Reaction {
                    target_sent_timestamp: Some(ts),
                    emoji: Some(emoji),
                    ..
                }),
            ..
        } => {
            let Ok(Some(message)) = manager.store().message(thread, *ts).await else {
                warn!(%thread, sent_at = ts, "no message found in thread");
                return None;
            };

            let ContentBody::DataMessage(DataMessage {
                body: Some(body), ..
            }) = message.body
            else {
                warn!("message reacted to has no body");
                return None;
            };

            Some(format!("Reacted with {emoji} to message: \"{body}\""))
        }
        DataMessage {
            body: Some(body), ..
        } => Some(body.to_string()),
        _ => Some("Empty data message".to_string()),
    }
}

pub async fn format_contact<S: Store>(uuid: &Uuid, manager: &Manager<S, Registered>) -> String {
    manager
        .store()
        .contact_by_id(uuid)
        .await
        .ok()
        .flatten()
        .filter(|c| !c.name.is_empty())
        .map(|c| format!("{},{}", c.name, uuid))
        .unwrap_or_else(|| uuid.to_string())
}

pub async fn format_group<S: Store>(key: [u8; 32], manager: &Manager<S, Registered>) -> String {
    manager
        .store()
        .group(key)
        .await
        .ok()
        .flatten()
        .map(|g| g.title)
        .unwrap_or_else(|| "<missing group>".to_string())
}
