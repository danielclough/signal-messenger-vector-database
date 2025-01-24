use std::path::Path;

use anyhow::Context as _;
use futures::pin_mut;
use futures::StreamExt;
use presage::model::messages::Received;
use presage::{manager::Registered, store::Store, Manager};
use sqlx::Pool;
use sqlx::Postgres;

use crate::signal::attachments_dir::attachments_dir;
use crate::signal::process_incoming_message::process_incoming_message;

pub async fn receive<S: Store>(
    manager: &mut Manager<S, Registered>,
    pg_pool: &Pool<Postgres>,
) -> anyhow::Result<()> {
    println!("Start contact");
    let attachments_dir = attachments_dir().await?;
    println!("{:?}", attachments_dir);
    let messages = manager
        .receive_messages()
        .await
        .context("failed to initialize messages stream")?;

    pin_mut!(messages);

    while let Some(content) = messages.next().await {
        // println!("{:?}",content);
        match content {
            Received::QueueEmpty => println!("done with synchronization"),
            Received::Contacts => println!("got contacts synchronization"),
            Received::Content(content) => {
                _ = process_incoming_message(
                    manager,
                    Path::new(&attachments_dir),
                    &content,
                    &pg_pool,
                )
                .await;
            }
        }
    }

    println!("Exit 0");
    Ok(())
}
