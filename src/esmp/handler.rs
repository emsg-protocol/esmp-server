use serde::Deserialize;
use serde_json::Value;
use crate::esmp::crypto::verify_signature;
use crate::esmp::group::persist_group_message;

#[derive(Debug, Deserialize)]
pub struct EsmpMessage {
    pub to: Vec<String>,
    pub cc: Option<Vec<String>>,
    pub group_id: Option<String>,
    pub r#type: String,
    pub subtype: Option<String>, // For system messages
    pub actor: Option<String>,   // For system messages
    pub target: Option<String>,  // For system messages
    pub timestamp: Option<u64>,  // For system messages
    pub body: Value,
    pub signature: String,
    pub sender_pubkey: String,
    // Optional system message metadata
    pub new_name: Option<String>,
    pub new_description: Option<String>,
    pub new_dp_url: Option<String>,
}

pub async fn handle_message(json: &str) {
    match serde_json::from_str::<EsmpMessage>(json) {
        Ok(msg) => {
            // Canonicalize for signature verification
            let to_sign = serde_json::json!({
                "to": msg.to,
                "cc": msg.cc,
                "group_id": msg.group_id,
                "type": msg.r#type,
                "subtype": msg.subtype,
                "actor": msg.actor,
                "target": msg.target,
                "timestamp": msg.timestamp,
                "body": msg.body,
                "new_name": msg.new_name,
                "new_description": msg.new_description,
                "new_dp_url": msg.new_dp_url,
            });
            let to_sign_bytes = serde_json::to_vec(&to_sign).unwrap();
            if verify_signature(&msg.sender_pubkey, &msg.signature, &to_sign_bytes) {
                if msg.r#type == "system" {
                    if let Some(group_id) = &msg.group_id {
                        persist_group_message(group_id, &msg).await;
                    }
                } else if let Some(group_id) = &msg.group_id {
                    persist_group_message(group_id, &msg).await;
                } else {
                    // Handle direct message (non-group)
                    println!("Direct message: {:?}", msg);
                }
            } else {
                eprintln!("Rejected unsigned or tampered ESMP message");
            }
        }
        Err(e) => {
            eprintln!("Invalid ESMP message: {}", e);
        }
    }
}
