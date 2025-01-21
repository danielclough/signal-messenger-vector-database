use presage::libsignal_service::proto::sync_message::Sent;
use presage::proto::receipt_message;
use presage::proto::EditMessage;
use presage::proto::ReceiptMessage;
use presage::proto::SyncMessage;
use presage::{
    libsignal_service::content::{Content, ContentBody},
    manager::Registered,
    store::{Store, Thread},
    Manager,
};
use tracing::warn;

use crate::signal::format::format_contact;
use crate::signal::format::format_data_message;
use crate::signal::format::format_group;

#[derive(Debug, Clone)]
pub enum Direction {
    To,
    From,
}
impl Direction {
    pub fn to_string(&self) -> String {
        match self {
            Direction::To => String::from("to"),
            Direction::From => String::from("from"),
        }
    }
}

#[derive(Debug)]
pub struct MessageEverything {
    pub direction: Option<Direction>,
    pub receiver: Option<String>,
    pub sender: Option<String>,
    pub group: Option<String>,
    pub body: Option<String>,
}

impl MessageEverything {
    pub fn default() -> MessageEverything {
        MessageEverything {
            direction: None,
            receiver: None,
            sender: None,
            group: None,
            body: None,
        }
    }
    pub fn error(error: String) -> MessageEverything {
        MessageEverything {
            direction: None,
            receiver: None,
            sender: None,
            group: None,
            body: Some(error),
        }
    }
}

pub async fn format_message<S: Store>(
    manager: &Manager<S, Registered>,
    content: &Content,
) -> MessageEverything {
    let Ok(thread) = Thread::try_from(content) else {
        let warning = "failed to derive thread from content";
        warn!(warning);
        return MessageEverything::error(warning.to_string());
    };

    enum Msg<'a> {
        Received(&'a Thread, String),
        Sent(&'a Thread, String),
    }

    if let Some(msg) = match &content.body {
        ContentBody::NullMessage(_) => Some(Msg::Received(
            &thread,
            "Null message (for example deleted)".to_string(),
        )),
        ContentBody::DataMessage(data_message) => {
            format_data_message(&thread, data_message, manager)
                .await
                .map(|body| Msg::Received(&thread, body))
        }
        ContentBody::EditMessage(EditMessage {
            data_message: Some(data_message),
            ..
        }) => format_data_message(&thread, data_message, manager)
            .await
            .map(|body| Msg::Received(&thread, body)),
        ContentBody::EditMessage(EditMessage { .. }) => None,
        ContentBody::SynchronizeMessage(SyncMessage {
            sent:
                Some(Sent {
                    message: Some(data_message),
                    ..
                }),
            ..
        }) => format_data_message(&thread, data_message, manager)
            .await
            .map(|body| Msg::Sent(&thread, body)),
        ContentBody::SynchronizeMessage(SyncMessage {
            sent:
                Some(Sent {
                    edit_message:
                        Some(EditMessage {
                            data_message: Some(data_message),
                            ..
                        }),
                    ..
                }),
            ..
        }) => format_data_message(&thread, data_message, manager)
            .await
            .map(|body| Msg::Sent(&thread, body)),
        ContentBody::SynchronizeMessage(SyncMessage { .. }) => None,
        ContentBody::CallMessage(_) => Some(Msg::Received(&thread, "is calling!".into())),
        ContentBody::TypingMessage(_) => Some(Msg::Received(&thread, "is typing...".into())),
        ContentBody::ReceiptMessage(ReceiptMessage {
            r#type: receipt_type,
            timestamp,
        }) => Some(Msg::Received(
            &thread,
            format!(
                "got {:?} receipt for messages sent at {timestamp:?}",
                receipt_message::Type::try_from(receipt_type.unwrap_or_default()).unwrap()
            ),
        )),
        ContentBody::StoryMessage(story) => {
            Some(Msg::Received(&thread, format!("new story: {story:?}")))
        }
        ContentBody::PniSignatureMessage(_) => {
            Some(Msg::Received(&thread, "got PNI signature message".into()))
        }
    } {
        match msg {
            Msg::Received(Thread::Contact(sender), body) => {
                let contact = format_contact(sender, manager).await;
                MessageEverything {
                    direction: Some(Direction::From),
                    receiver: Some(contact),
                    sender: None,
                    group: None,
                    body: Some(body),
                }
            }
            Msg::Sent(Thread::Contact(recipient), body) => {
                let contact = format_contact(recipient, manager).await;
                MessageEverything {
                    direction: Some(Direction::To),
                    receiver: Some(contact),
                    sender: None,
                    group: None,
                    body: Some(body),
                }
            }
            Msg::Received(Thread::Group(key), body) => {
                let sender = format_contact(&content.metadata.sender.raw_uuid(), manager).await;
                let group = format_group(*key, manager).await;
                MessageEverything {
                    direction: Some(Direction::From),
                    receiver: None,
                    sender: Some(sender),
                    group: Some(group),
                    body: Some(body),
                }
            }
            Msg::Sent(Thread::Group(key), body) => {
                let group = format_group(*key, manager).await;
                MessageEverything {
                    direction: Some(Direction::To),
                    receiver: None,
                    sender: None,
                    group: Some(group),
                    body: Some(body),
                }
            }
        }
    } else {
        MessageEverything::error(String::from("Something went wrong!"))
    }
}
