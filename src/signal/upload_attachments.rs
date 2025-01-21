use mime_guess::mime::APPLICATION_OCTET_STREAM;
use presage::libsignal_service::sender::AttachmentSpec;
use presage::{manager::Registered, store::Store, Manager};
use std::path::PathBuf;

pub async fn upload_attachments<S: Store>(
    attachment_filepath: Vec<PathBuf>,
    manager: &Manager<S, Registered>,
) -> Result<Vec<presage::proto::AttachmentPointer>, anyhow::Error> {
    let attachment_specs: Vec<_> = attachment_filepath
        .into_iter()
        .filter_map(|path| {
            let data = std::fs::read(&path).ok()?;
            Some((
                AttachmentSpec {
                    content_type: mime_guess::from_path(&path)
                        .first()
                        .unwrap_or(APPLICATION_OCTET_STREAM)
                        .to_string(),
                    length: data.len(),
                    file_name: path.file_name().map(|s| s.to_string_lossy().to_string()),
                    preview: None,
                    voice_note: None,
                    borderless: None,
                    width: None,
                    height: None,
                    caption: None,
                    blur_hash: None,
                },
                data,
            ))
        })
        .collect();

    let attachments: Result<Vec<_>, _> = manager
        .upload_attachments(attachment_specs)
        .await?
        .into_iter()
        .collect();

    let attachments = attachments?;
    Ok(attachments)
}
