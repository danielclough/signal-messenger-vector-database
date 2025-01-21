pub mod attachments_dir;
pub mod format;
pub mod format_message;
pub mod process_incoming_message;
pub mod receive;
pub mod send;
pub mod upload_attachments;

// fn parse_group_master_key(value: &str) -> anyhow::Result<GroupMasterKeyBytes> {
//     let master_key_bytes = hex::decode(value)?;
//     master_key_bytes
//         .try_into()
//         .map_err(|_| anyhow::format_err!("master key should be 32 bytes long"))
// }

// fn parse_base64_profile_key(s: &str) -> anyhow::Result<ProfileKey> {
//     let bytes = BASE64_STANDARD
//         .decode(s)?
//         .try_into()
//         .map_err(|_| anyhow!("profile key of invalid length"))?;
//     Ok(ProfileKey::create(bytes))
// }
