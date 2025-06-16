use crate::esmp::handler::EsmpMessage;
use tokio::fs::{OpenOptions, File};
use tokio::io::{AsyncWriteExt, AsyncBufReadExt, BufReader};
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct GroupMetadata {
    pub group_id: String,
    pub group_name: Option<String>,
    pub group_description: Option<String>,
    pub group_display_picture: Option<String>,
}

pub async fn persist_group_message(group_id: &str, msg: &EsmpMessage) {
    let thread_file = format!("group_{}.jsonl", group_id);
    let record = serde_json::to_string(msg).unwrap();
    let _ = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&thread_file)
        .await
        .and_then(|mut file| AsyncWriteExt::write_all(&mut file, format!("{}\n", record).as_bytes()));

    match msg.r#type.as_str() {
        "group_created" | "group_updated" => {
            // Extract metadata fields from body
            let meta = GroupMetadata {
                group_id: group_id.to_string(),
                group_name: msg.body.get("group_name").and_then(|v| v.as_str()).map(|s| s.to_string()),
                group_description: msg.body.get("group_description").and_then(|v| v.as_str()).map(|s| s.to_string()),
                group_display_picture: msg.body.get("group_display_picture").and_then(|v| v.as_str()).map(|s| s.to_string()),
            };
            let meta_file = format!("group_{}_meta.json", group_id);
            let _ = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&meta_file)
                .await
                .and_then(|mut file| AsyncWriteExt::write_all(&mut file, serde_json::to_string_pretty(&meta).unwrap().as_bytes()));
            println!("Persisted group metadata for group {}: {:?}", group_id, meta);
        }
        "joined" | "left" | "removed" => {
            println!("System message for group {}: {}", group_id, msg.r#type);
        }
        _ => {
            println!("Persisted group message for group {}", group_id);
        }
    }
}

pub async fn fetch_group_metadata(group_id: &str) -> Option<GroupMetadata> {
    let meta_file = format!("group_{}_meta.json", group_id);
    match tokio::fs::read_to_string(&meta_file).await {
        Ok(content) => serde_json::from_str(&content).ok(),
        Err(_) => None,
    }
}

pub async fn list_group_messages(group_id: &str) -> Option<(GroupMetadata, Vec<EsmpMessage>)> {
    let meta = fetch_group_metadata(group_id).await?;
    let thread_file = format!("group_{}.jsonl", group_id);
    let file = File::open(&thread_file).await.ok()?;
    let mut reader = BufReader::new(file);
    let mut messages = Vec::new();
    let mut line = String::new();
    while reader.read_line(&mut line).await.ok()? > 0 {
        if let Ok(msg) = serde_json::from_str::<EsmpMessage>(&line) {
            messages.push(msg);
        }
        line.clear();
    }
    Some((meta, messages))
}
