pub mod types;
pub mod signal;
pub mod rag;

use std::convert::TryInto;

use anyhow::{bail, Context as _};
use directories::ProjectDirs;
use env_logger::Env;
use futures::StreamExt;
use futures::{channel::oneshot, future, pin_mut};
use presage::libsignal_service::pre_keys::PreKeysStore;
use presage::libsignal_service::prelude::ProfileKey;
use presage::model::contacts::Contact;
use presage::model::groups::Group;
use presage::model::identity::OnNewIdentity;
use presage::model::messages::Received;
use presage::{
    libsignal_service::content::{DataMessage, GroupContextV2},
    manager::RegistrationOptions,
    store::{Store, Thread},
    Manager,
};
use presage_store_sled::MigrationConflictStrategy;
use presage_store_sled::SledStore;
use sqlx::{Pool, Postgres};
use tokio::io::{self, AsyncBufReadExt, BufReader};
use tracing::{debug, error};
use types::Args;
use types::Cmd;
use types::Recipient;
use signal::format_message::format_message;
use signal::receive::receive;
use signal::send::send;
use signal::upload_attachments::upload_attachments;

pub async fn entry_point(args: Args, pg_pool: &Pool<Postgres>) -> anyhow::Result<String> {
    env_logger::Builder::from_env(
        Env::default().default_filter_or(format!("{}=warn", env!("CARGO_PKG_NAME"))),
    )
    .init();

    let db_path = args.db_path.unwrap_or_else(|| {
        ProjectDirs::from("org", "whisperfish", "presage")
            .unwrap()
            .config_dir()
            .into()
    });
    debug!(db_path =% db_path.display(), "opening config database");
    let config_store = SledStore::open_with_passphrase(
        db_path,
        args.passphrase,
        MigrationConflictStrategy::Raise,
        OnNewIdentity::Trust,
    )
    .await?;
    run(args.subcommand, config_store, pg_pool).await
}

async fn run<S: Store>(subcommand: Cmd, config_store: S, pg_pool: &Pool<Postgres>) -> anyhow::Result<String> {
    let mut response = String::new();

    match subcommand {
        Cmd::Register {
            servers,
            phone_number,
            use_voice_call,
            captcha,
            force,
        } => {
            let manager = Manager::register(
                config_store,
                RegistrationOptions {
                    signal_servers: servers,
                    phone_number,
                    use_voice_call,
                    captcha: Some(captcha.host_str().unwrap()),
                    force,
                },
            )
            .await?;

            // ask for confirmation code here
            response = format!("input confirmation code (followed by RETURN): ");
            let stdin = io::stdin();
            let reader = BufReader::new(stdin);
            if let Some(confirmation_code) = reader.lines().next_line().await? {
                let registered_manager =
                    manager.confirm_verification_code(confirmation_code).await?;
                response = format!(
                    "Account identifiers: {}",
                    registered_manager.registration_data().service_ids
                );
            }
        }
        Cmd::LinkDevice {
            servers,
            device_name,
        } => {
            let (provisioning_link_tx, provisioning_link_rx) = oneshot::channel();
            let manager = future::join(
                Manager::link_secondary_device(
                    config_store,
                    servers,
                    device_name.clone(),
                    provisioning_link_tx,
                ),
                async move {
                    match provisioning_link_rx.await {
                        Ok(url) => {
                            println!("Please scan in the QR code:");
                            qr2term::print_qr(url.to_string()).expect("failed to render qrcode");
                            println!("Alternatively, use the URL: {}", url);
                        }
                        Err(error) => error!(%error, "linking device was cancelled"),
                    }
                },
            )
            .await;

            match manager {
                (Ok(manager), _) => {
                    let whoami = manager.whoami().await.unwrap();
                    response = format!("{whoami:?}");
                }
                (Err(err), _) => {
                    response = format!("{err:?}");
                }
            }
        }
        Cmd::AddDevice { url } => {
            let mut manager = Manager::load_registered(config_store).await?;
            manager.link_secondary(url).await?;
            response = format!("Added new secondary device");
        }
        Cmd::UnlinkDevice { device_id } => {
            let manager = Manager::load_registered(config_store).await?;
            manager.unlink_secondary(device_id).await?;
            response = format!("Unlinked device with id: {}", device_id);
        }
        Cmd::ListDevices => {
            let manager = Manager::load_registered(config_store).await?;
            let devices = manager.devices().await?;
            let current_device_id = manager.device_id() as i64;

            for device in devices {
                let device_name = device
                    .name
                    .unwrap_or_else(|| "(no device name)".to_string());
                let current_marker = if device.id == current_device_id {
                    "(this device)"
                } else {
                    ""
                };

                response = format!(
                    "- Device {} {}\n  Name: {}\n  Created: {}\n  Last seen: {}",
                    device.id, current_marker, device_name, device.created, device.last_seen,
                );
            }
        }
        Cmd::Receive { notifications } => {
            let mut manager = Manager::load_registered(config_store).await?;
            receive(&mut manager, notifications, pg_pool).await?;
            response = "Receiver Exiting".to_string();
        }
        Cmd::Send {
            uuid,
            message,
            attachment_filepath,
        } => {
            let mut manager = Manager::load_registered(config_store).await?;
            let attachments = upload_attachments(attachment_filepath, &manager).await?;
            let data_message = DataMessage {
                body: Some(message),
                attachments,
                ..Default::default()
            };

            send(&mut manager, Recipient::Contact(uuid), data_message, pg_pool).await?;
        }
        Cmd::SendToGroup {
            message,
            master_key,
            attachment_filepath,
        } => {
            let mut manager = Manager::load_registered(config_store).await?;
            let attachments = upload_attachments(attachment_filepath, &manager).await?;
            let data_message = DataMessage {
                body: Some(message),
                attachments,
                group_v2: Some(GroupContextV2 {
                    master_key: Some(master_key.to_vec()),
                    revision: Some(0),
                    ..Default::default()
                }),
                ..Default::default()
            };

            send(&mut manager, Recipient::Group(master_key), data_message, pg_pool).await?;
        }
        Cmd::RetrieveProfile {
            uuid,
            mut profile_key,
        } => {
            let mut manager = Manager::load_registered(config_store).await?;
            if profile_key.is_none() {
                for contact in manager
                    .store()
                    .contacts()
                    .await?
                    .filter_map(Result::ok)
                    .filter(|c| c.uuid == uuid)
                {
                    let profilek:[u8;32] = match(contact.profile_key).try_into() {
                    Ok(profilek) => profilek,
                    Err(_) => bail!("Profile key is not 32 bytes or empty for uuid: {:?} and no alternative profile key was provided", uuid),
                };
                    profile_key = Some(ProfileKey::create(profilek));
                }
            } else {
                println!("Retrieving profile for: {uuid:?} with profile_key");
            }
            let profile = match profile_key {
                None => manager.retrieve_profile().await?,
                Some(profile_key) => manager.retrieve_profile_by_uuid(uuid, profile_key).await?,
            };
            response = format!("{profile:#?}");
        }
        Cmd::ListGroups => {
            let manager = Manager::load_registered(config_store).await?;
            for group in manager.store().groups().await? {
                match group {
                    Ok((
                        group_master_key,
                        Group {
                            title,
                            description,
                            revision,
                            members,
                            ..
                        },
                    )) => {
                        let key = hex::encode(group_master_key);
                        response = format!(
                            "{key} {title}: {description:?} / revision {revision} / {} members",
                            members.len()
                        );
                    }
                    Err(error) => {
                        error!(%error, "failed to deserialize group");
                    }
                };
            }
        }
        Cmd::ListContacts => {
            let manager = Manager::load_registered(config_store).await?;
            for Contact {
                name,
                uuid,
                phone_number,
                ..
            } in manager.store().contacts().await?.flatten()
            {
                response = format!("{uuid} / {phone_number:?} / {name}");
            }
        }
        Cmd::ListStickerPacks => {
            let manager = Manager::load_registered(config_store).await?;
            for sticker_pack in manager.store().sticker_packs().await? {
                match sticker_pack {
                    Ok(sticker_pack) => {
                        response = format!(
                            "title={} author={}",
                            sticker_pack.manifest.title, sticker_pack.manifest.author,
                        );
                        for sticker in sticker_pack.manifest.stickers {
                            response = format!(
                                "\tid={} emoji={} content_type={} bytes={}",
                                sticker.id,
                                sticker.emoji.unwrap_or_default(),
                                sticker.content_type.unwrap_or_default(),
                                sticker.bytes.unwrap_or_default().len(),
                            )
                        }
                    }
                    Err(error) => {
                        error!(%error, "error while deserializing sticker pack")
                    }
                }
            }
        }
        Cmd::Whoami => {
            let manager = Manager::load_registered(config_store).await?;
            response = format!("{:?}", &manager.whoami().await?);
        }
        Cmd::GetContact { ref uuid } => {
            let manager = Manager::load_registered(config_store).await?;
            match manager.store().contact_by_id(uuid).await? {
                Some(contact) => response = format!("{contact:#?}"),
                None => response = format!("Could not find contact for {uuid}"),
            }
        }
        Cmd::FindContact {
            uuid,
            phone_number,
            ref name,
        } => {
            let manager = Manager::load_registered(config_store).await?;
            for contact in manager
                .store()
                .contacts()
                .await?
                .filter_map(Result::ok)
                .filter(|c| uuid.map_or_else(|| true, |u| c.uuid == u))
                .filter(|c| c.phone_number == phone_number)
                .filter(|c| name.as_ref().map_or(true, |n| c.name.contains(n)))
            {
                response = format!("{contact:#?}");
            }
        }
        Cmd::SyncContacts => {
            let mut manager = Manager::load_registered(config_store).await?;
            manager.request_contacts().await?;

            let messages = manager
                .receive_messages()
                .await
                .context("failed to initialize messages stream")?;
            pin_mut!(messages);

            response = format!("synchronizing messages until we get contacts (dots are messages synced from the past timeline)");

            while let Some(content) = messages.next().await {
                match content {
                    Received::QueueEmpty => break,
                    Received::Contacts => {
                        response = format!("got contacts! thank you, come again.")
                    }
                    Received::Content(_) => print!("."),
                }
            }
        }
        Cmd::ListMessages {
            group_master_key,
            recipient_uuid,
            from,
        } => {
            let manager = Manager::load_registered(config_store).await?;
            let thread = match (group_master_key, recipient_uuid) {
                (Some(master_key), _) => Thread::Group(master_key),
                (_, Some(uuid)) => Thread::Contact(uuid),
                _ => unreachable!(),
            };
            for msg in manager
                .store()
                .messages(&thread, from.unwrap_or(0)..)
                .await?
                .filter_map(Result::ok)
            {
                format_message(&manager,  &msg).await;
            }
        }
        Cmd::Stats => {
            let manager = Manager::load_registered(config_store).await?;

            #[allow(unused)]
            #[derive(Debug)]
            struct Stats {
                aci_next_pre_key_id: u32,
                aci_next_signed_pre_keys_id: u32,
                aci_next_kyber_pre_keys_id: u32,
                aci_signed_pre_keys_count: usize,
                aci_kyber_pre_keys_count: usize,
                aci_kyber_pre_keys_count_last_resort: usize,
                pni_next_pre_key_id: u32,
                pni_next_signed_pre_keys_id: u32,
                pni_next_kyber_pre_keys_id: u32,
                pni_signed_pre_keys_count: usize,
                pni_kyber_pre_keys_count: usize,
                pni_kyber_pre_keys_count_last_resort: usize,
            }

            let aci = manager.store().aci_protocol_store();
            let pni = manager.store().pni_protocol_store();

            const LAST_RESORT: bool = true;

            let stats = Stats {
                aci_next_pre_key_id: aci.next_pre_key_id().await.unwrap(),
                aci_next_signed_pre_keys_id: aci.next_signed_pre_key_id().await.unwrap(),
                aci_next_kyber_pre_keys_id: aci.next_pq_pre_key_id().await.unwrap(),
                aci_signed_pre_keys_count: aci.signed_pre_keys_count().await.unwrap(),
                aci_kyber_pre_keys_count: aci.kyber_pre_keys_count(!LAST_RESORT).await.unwrap(),
                aci_kyber_pre_keys_count_last_resort: aci
                    .kyber_pre_keys_count(LAST_RESORT)
                    .await
                    .unwrap(),
                pni_next_pre_key_id: pni.next_pre_key_id().await.unwrap(),
                pni_next_signed_pre_keys_id: pni.next_signed_pre_key_id().await.unwrap(),
                pni_next_kyber_pre_keys_id: pni.next_pq_pre_key_id().await.unwrap(),
                pni_signed_pre_keys_count: pni.signed_pre_keys_count().await.unwrap(),
                pni_kyber_pre_keys_count: pni.kyber_pre_keys_count(!LAST_RESORT).await.unwrap(),
                pni_kyber_pre_keys_count_last_resort: pni
                    .kyber_pre_keys_count(LAST_RESORT)
                    .await
                    .unwrap(),
            };

            response = format!("{stats:#?}")
        }
    }

    // println!("{}",response);
    Ok(response)
}
