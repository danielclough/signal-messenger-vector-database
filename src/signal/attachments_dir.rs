use std::fs;

use tracing::info;

pub async fn attachments_dir() -> anyhow::Result<String> {
    let attachments_dir = "attachments";
    let exists = tokio::fs::try_exists(&attachments_dir)
        .await
        .unwrap_or(false);
    if !exists {
        _ = fs::create_dir(&attachments_dir).expect("ERROR: create attachments dir failed");
    }
    info!(
        path =% attachments_dir,
        "attachments will be stored"
    );
    Ok(attachments_dir.to_string())
}
