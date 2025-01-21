use std::path::Path;
use std::time::Duration;
use std::time::UNIX_EPOCH;

use anyhow::Context as _;
use futures::pin_mut;
use futures::StreamExt;
use presage::libsignal_service::protocol::ServiceId;
use presage::model::messages::Received;
use presage::{
    libsignal_service::content::ContentBody, manager::Registered, store::Store, Manager,
};
use sqlx::Pool;
use sqlx::Postgres;
use tracing::info;

use crate::types::Recipient;
use crate::signal::attachments_dir::attachments_dir;
use crate::signal::process_incoming_message::process_incoming_message;

pub async fn send<S: Store>(
    manager: &mut Manager<S, Registered>,
    recipient: Recipient,
    msg: impl Into<ContentBody>,
    pg_pool: &Pool<Postgres>,
) -> anyhow::Result<()> {
    let attachments_dir = attachments_dir().await?;

    let timestamp = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis() as u64;

    let mut content_body = msg.into();
    if let ContentBody::DataMessage(d) = &mut content_body {
        d.timestamp = Some(timestamp);
    }

    let messages = manager
        .receive_messages()
        .await
        .context("failed to initialize messages stream")?;
    pin_mut!(messages);

    println!("synchronizing messages since last time");

    while let Some(content) = messages.next().await {
        match content {
            Received::QueueEmpty => break,
            Received::Contacts => continue,
            Received::Content(content) => {
                _ = process_incoming_message(
                    manager,
                    Path::new(&attachments_dir),
                    false,
                    &content,
                    pg_pool
                )
                .await;
            }
        }
    }

    println!("done synchronizing, sending your message now!");

    match recipient {
        Recipient::Contact(uuid) => {
            info!(recipient =% uuid, "sending message to contact");
            manager
                .send_message(ServiceId::Aci(uuid.into()), content_body, timestamp)
                .await
                .expect("failed to send message");
        }
        Recipient::Group(master_key) => {
            info!("sending message to group");
            manager
                .send_message_to_group(&master_key, content_body, timestamp)
                .await
                .expect("failed to send message");
        }
    }

    tokio::time::timeout(Duration::from_secs(60), async move {
        while let Some(msg) = messages.next().await {
            if let Received::Contacts = msg {
                println!("got contacts sync!");
                break;
            }
        }
    })
    .await?;

    Ok(())
}
