use chrono::Local;
use presage::{
    libsignal_service::content::{Content, ContentBody, DataMessage},
    manager::Registered,
    store::Store,
    Manager,
};
use sqlx::{Pool, Postgres};
use std::path::Path;
use tokio::fs;
use tracing::warn;
use tracing::{error, info};

use crate::rag::{dataframes::process_dataframe, sqlx::insert_embeddings_into_db};

use super::format_message::{format_message, Direction, MessageEverything};

#[derive(Debug, Clone)]
pub struct ProcessedMessage {
    pub direction: Option<Direction>,
    pub contact: Option<String>,
    pub sender: Option<String>,
    pub group: Option<String>,
    pub body: Option<String>,
    pub attachments: Option<Vec<String>>,
}

// Note to developers, this is a good example of a function you can use as a source of inspiration
// to process incoming messages.
pub async fn process_incoming_message<S: Store>(
    manager: &mut Manager<S, Registered>,
    attachments_dir: &Path,
    content: &Content,
    pg_pool: &Pool<Postgres>,
) -> ProcessedMessage {
    let MessageEverything {
        direction,
        contact,
        group,
        body,
    } = format_message(manager, content).await;
    // println!("{}\n{}\n",msg_prefix,msg_content);
    let mut path_vec = vec![];

    let sender = content.metadata.sender.raw_uuid();
    if let ContentBody::DataMessage(DataMessage { attachments, .. }) = &content.body {
        for attachment_pointer in attachments {
            let Ok(attachment_data) = manager.get_attachment(attachment_pointer).await else {
                warn!("failed to fetch attachment");
                continue;
            };

            let extensions = mime_guess::get_mime_extensions_str(
                attachment_pointer
                    .content_type
                    .as_deref()
                    .unwrap_or("application/octet-stream"),
            );
            let extension = extensions.and_then(|e| e.first()).unwrap_or(&"bin");
            let timestamp = Local::now().format("%Y-%m-%d-%H-%M-%s").to_string();
            let default_name = format!("{}.{}", &timestamp, extension);
            let mut filename: String = attachment_pointer
                .file_name
                .clone()
                .unwrap_or_else(|| default_name.clone());

            if filename != default_name {
                filename = format!("{}-{}", timestamp, filename);
            }

            path_vec.push(filename.clone());
            let file_path = attachments_dir.join(filename);
            match fs::write(&file_path, &attachment_data).await {
                Ok(_) => info!(%sender, file_path =% file_path.display(), "saved attachment"),
                Err(error) => error!(
                    %sender,
                    file_path =% file_path.display(),
                    %error,
                    "failed to write attachment"
                ),
            }
        }
    }

    let processed_message = ProcessedMessage {
        attachments: if path_vec.len() > 0 {
            Some(path_vec)
        } else {
            None
        },
        direction,
        contact,
        sender: Some(sender.to_string()),
        group,
        body,
    };

    store_in_db(processed_message.clone(), pg_pool).await;

    processed_message
}

pub async fn store_in_db(processed_message: ProcessedMessage, pg_pool: &Pool<Postgres>) {
    let msg = processed_message.body.clone().unwrap_or(String::new());

    match msg.as_str() {
        "failed to derive thread from content" => (),
        "Null message (for example deleted)" => (),
        "is calling!" => (),
        "is typing..." => (),
        "got PNI signature message" => (),
        "Empty data message" => (),
        "presage" => (),
        "failed to display desktop notification" => (),
        "Something went wrong!" => (),
        s if s.starts_with("got Delivery receipt") => (),
        s if s.starts_with("got Read receipt") => (),
        s if s.starts_with("new story:") => (),
        s if s.starts_with("receipt for messages sent at") => (),
        s if s.starts_with("Reacted with ") => (),
        _ => {
            // println!("{:#?}", msg);

            let messages_with_embedding = process_dataframe(&vec![processed_message.clone()]).await;

            _ = insert_embeddings_into_db(pg_pool, messages_with_embedding).await;
        }
    }
    ()
}
