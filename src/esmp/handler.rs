use serde::Deserialize;
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::esmp::crypto::verify_signature;
use crate::esmp::group::{persist_group_message, fetch_group_metadata, GroupMetadata};
use crate::esmp::system::SystemMessageType;

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

impl EsmpMessage {
    fn validate_system_message(&self) -> Result<(), &'static str> {
        // System messages must have a subtype
        let subtype = self.subtype.as_ref()
            .ok_or("System messages must have a subtype")?;
            
        let sys_type = SystemMessageType::from_str(subtype)
            .ok_or("Invalid system message subtype")?;

        // All current system messages require a group
        if sys_type.requires_group() && self.group_id.is_none() {
            return Err("This system message type requires a group_id");
        }

        // Validate actor field
        if sys_type.requires_actor() && self.actor.is_none() {
            return Err("This system message type requires an actor");
        }

        // Validate target field
        if sys_type.requires_target() && self.target.is_none() {
            return Err("This system message type requires a target");
        }

        // Validate metadata based on subtype
        match sys_type {
            SystemMessageType::GroupRenamed => {
                if self.new_name.is_none() {
                    return Err("group_renamed requires new_name");
                }
            }
            SystemMessageType::DescriptionUpdated => {
                if self.new_description.is_none() {
                    return Err("description_updated requires new_description");
                }
            }
            SystemMessageType::DpUpdated => {
                if self.new_dp_url.is_none() {
                    return Err("dp_updated requires new_dp_url");
                }
            }
            _ => {}
        }

        Ok(())
    }
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
                    // Validate and process system message
                    match msg.validate_system_message() {
                        Ok(()) => {
                            let group_id = msg.group_id.as_ref().expect("group_id already validated");
                            let now = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_secs();
                            
                            let mut message = msg.clone();
                            if message.timestamp.is_none() {
                                message.timestamp = Some(now);
                            }

                            // Handle system message based on subtype
                            let subtype = message.subtype.as_ref().unwrap();
                            match SystemMessageType::from_str(subtype).unwrap() {
                                SystemMessageType::AdminAssigned | SystemMessageType::AdminRevoked => {
                                    // Update group metadata for admin changes
                                    if let Some(mut metadata) = fetch_group_metadata(group_id).await {
                                        let target = message.target.as_ref().unwrap();
                                        if subtype == "admin_assigned" && !metadata.admins.contains(target) {
                                            metadata.admins.push(target.clone());
                                        } else if subtype == "admin_revoked" {
                                            metadata.admins.retain(|x| x != target);
                                        }
                                        metadata.updated_at = Some(now);
                                    }
                                }
                                _ => () // Other system messages handled by persist_group_message
                            }

                            persist_group_message(group_id, &message).await;
                        }
                        Err(e) => {
                            eprintln!("Invalid system message: {}", e);
                            return;
                        }
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
