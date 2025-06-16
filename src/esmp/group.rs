use crate::esmp::handler::EsmpMessage;
use tokio::fs::{OpenOptions, File};
use tokio::io::{AsyncWriteExt, AsyncBufReadExt, BufReader};
use serde::{Serialize, Deserialize};
use std::time::{SystemTime, UNIX_EPOCH};
use std::str::FromStr;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct GroupMetadata {
    pub group_id: String,
    pub group_name: Option<String>,
    pub group_description: Option<String>,
    pub group_display_picture: Option<String>,
    pub created_at: Option<u64>,
    pub updated_at: Option<u64>,
    pub admins: Vec<String>,
    pub members: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SystemMessageType;

impl SystemMessageType {
    pub fn from_str(s: &str) -> Result<Self, ()> {
        // Dummy implementation for the sake of example
        Ok(SystemMessageType)
    }
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

    // Load or create group metadata
    let mut metadata = fetch_group_metadata(group_id)
        .await
        .unwrap_or_else(|| GroupMetadata {
            group_id: group_id.to_string(),
            ..Default::default()
        });

    // Update metadata based on message type
    if msg.r#type == "system" {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        if let Some(subtype) = &msg.subtype {
            match SystemMessageType::from_str(subtype).unwrap() {
                SystemMessageType::GroupCreated => {
                    metadata.created_at = Some(now);
                    metadata.updated_at = Some(now);
                    metadata.group_name = msg.body.get("group_name").and_then(|v| v.as_str()).map(|s| s.to_string());
                    metadata.group_description = msg.body.get("group_description").and_then(|v| v.as_str()).map(|s| s.to_string());
                    metadata.group_display_picture = msg.body.get("group_display_picture").and_then(|v| v.as_str()).map(|s| s.to_string());
                    if let Some(actor) = &msg.actor {
                        metadata.admins.push(actor.clone());
                        metadata.members.push(actor.clone());
                    }
                }
                SystemMessageType::GroupRenamed => {
                    if let Some(new_name) = &msg.new_name {
                        metadata.group_name = Some(new_name.clone());
                        metadata.updated_at = Some(now);
                    }
                }
                SystemMessageType::DescriptionUpdated => {
                    if let Some(new_desc) = &msg.new_description {
                        metadata.group_description = Some(new_desc.clone());
                        metadata.updated_at = Some(now);
                    }
                }
                SystemMessageType::DpUpdated => {
                    if let Some(new_dp) = &msg.new_dp_url {
                        metadata.group_display_picture = Some(new_dp.clone());
                        metadata.updated_at = Some(now);
                    }
                }
                SystemMessageType::Joined => {
                    if let Some(actor) = &msg.actor {
                        if !metadata.members.contains(actor) {
                            metadata.members.push(actor.clone());
                            metadata.updated_at = Some(now);
                        }
                    }
                }
                SystemMessageType::Left | SystemMessageType::Removed => {
                    if let Some(target) = msg.target.as_ref().or(msg.actor.as_ref()) {
                        metadata.members.retain(|x| x != target);
                        metadata.admins.retain(|x| x != target);
                        metadata.updated_at = Some(now);
                    }
                }
                SystemMessageType::AdminAssigned => {
                    if let Some(target) = &msg.target {
                        if !metadata.admins.contains(target) {
                            metadata.admins.push(target.clone());
                            metadata.updated_at = Some(now);
                        }
                    }
                }
                SystemMessageType::AdminRevoked => {
                    if let Some(target) = &msg.target {
                        metadata.admins.retain(|x| x != target);
                        metadata.updated_at = Some(now);
                    }
                }
            }
        }

        // Save updated metadata
        let meta_file = format!("group_{}_meta.json", group_id);
        let _ = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&meta_file)
            .await
            .and_then(|mut file| AsyncWriteExt::write_all(&mut file, 
                serde_json::to_string_pretty(&metadata).unwrap().as_bytes()));

        println!("Updated group metadata for {}: {:?}", group_id, metadata);
    } else {
        println!("Persisted group message for group {}", group_id);
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
