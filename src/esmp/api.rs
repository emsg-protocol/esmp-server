use actix_web::{web, HttpResponse, http::StatusCode};
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use crate::esmp::{
    group::{fetch_group_metadata, GroupMetadata},
    handler::EsmpMessage,
    system::SystemMessageType,
    crypto::verify_signature,
    profile::{UserProfile, save_profile, ProfileError, Visibility},
};

#[derive(Debug, Serialize)]
struct GroupResponse {
    group_id: String,
    name: Option<String>,
    description: Option<String>, 
    display_picture_url: Option<String>,
    admins: Vec<String>,
    members: Vec<String>,
    created_at: Option<u64>,
    updated_at: Option<u64>
}

#[derive(Debug, Deserialize)]
pub struct GroupUpdateRequest {
    name: Option<String>,
    description: Option<String>,
    display_picture_url: Option<String>
}

#[derive(Debug, Deserialize)]
struct SignedRequest {
    request: GroupUpdateRequest,
    signature: String,
    pubkey: String
}

#[derive(Debug, Deserialize)]
pub struct ProfileFieldUpdate<T> {
    pub value: Option<T>,
    pub visibility: Option<Visibility>,
}

#[derive(Debug, Deserialize)]
pub struct ProfileUpdateRequest {
    first_name: Option<ProfileFieldUpdate<String>>,
    middle_name: Option<ProfileFieldUpdate<String>>,
    last_name: Option<ProfileFieldUpdate<String>>,
    display_picture: Option<ProfileFieldUpdate<String>>,
    address: Option<ProfileFieldUpdate<String>>,
}

#[derive(Debug, Serialize)]
pub struct ProfileResponse {
    pubkey: String,
    first_name: Option<String>,
    middle_name: Option<String>,
    last_name: Option<String>,
    display_picture: Option<String>,
    address: Option<String>,
    updated_at: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct SignedProfileRequest {
    request: ProfileUpdateRequest,
    signature: String,
    pubkey: String,
}

impl ProfileUpdateRequest {
    fn validate(&self) -> Result<(), String> {
        // Ensure address visibility is not set to public
        if let Some(addr) = &self.address {
            if matches!(addr.visibility, Some(Visibility::Public)) {
                return Err("Address field cannot be marked as public".into());
            }
        }

        // Validate display picture URL if present
        if let Some(dp) = &self.display_picture {
            if let Some(url) = &dp.value {
                if let Err(e) = Url::parse(url) {
                    return Err(format!("Invalid display picture URL: {}", e));
                }
            }
        }

        // Name fields length and character validation
        for (field_name, field) in [
            ("first_name", &self.first_name),
            ("middle_name", &self.middle_name),
            ("last_name", &self.last_name),
        ] {
            if let Some(field) = field {
                if let Some(value) = &field.value {
                    if value.len() > MAX_NAME_LENGTH {
                        return Err(format!("{} is too long (max {} characters)", 
                            field_name, MAX_NAME_LENGTH));
                    }
                    if let Some(invalid_char) = value.chars().find(|c| {
                        !c.is_alphabetic() && *c != ' ' && *c != '-' && *c != '\'
                    }) {
                        return Err(format!("Invalid character '{}' in {}", 
                            invalid_char, field_name));
                    }
                }
            }
        }

        Ok(())
    }
}

pub async fn get_group_metadata(group_id: web::Path<String>) -> HttpResponse {
    match fetch_group_metadata(&group_id).await {
        Some(metadata) => {
            let response = GroupResponse {
                group_id: metadata.group_id,
                name: metadata.group_name,
                description: metadata.group_description,
                display_picture_url: metadata.group_dp_url,
                admins: metadata.admins,
                members: metadata.members,
                created_at: metadata.created_at,
                updated_at: metadata.updated_at,
            };
            HttpResponse::Ok().json(response)
        }
        None => HttpResponse::NotFound().finish()
    }
}

pub async fn update_group_metadata(
    group_id: web::Path<String>,
    req: web::Json<SignedRequest>,
) -> HttpResponse {
    // Verify the request is signed by an admin
    if let Some(metadata) = fetch_group_metadata(&group_id).await {
        if !metadata.admins.iter().any(|admin| admin == &req.pubkey) {
            return HttpResponse::Forbidden().json(serde_json::json!({
                "error": "Only group administrators can modify group settings"
            }));
        }

        // Verify the signature
        let request_bytes = serde_json::to_vec(&req.request).unwrap();
        if !verify_signature(&req.pubkey, &req.signature, &request_bytes) {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid signature"
            }));
        }

        // Process updates and emit system messages
        let now = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut updates = Vec::new();

        if let Some(new_name) = &req.request.name {
            updates.push(("group_renamed", "new_name", new_name.clone()));
        }

        if let Some(new_desc) = &req.request.description {
            updates.push(("description_updated", "new_description", new_desc.clone()));
        }

        if let Some(new_dp) = &req.request.display_picture_url {
            updates.push(("dp_updated", "new_dp_url", new_dp.clone()));
        }

        // Emit system messages for each change
        for (subtype, field_name, value) in updates {
            let msg = EsmpMessage {
                to: metadata.members.clone(),
                cc: None, 
                group_id: Some(group_id.to_string()),
                r#type: "system".to_string(),
                subtype: Some(subtype.to_string()),
                actor: Some(req.pubkey.clone()),
                target: None,
                timestamp: Some(now),
                body: serde_json::json!({}),
                signature: req.signature.clone(),
                sender_pubkey: req.pubkey.clone(),
                new_name: if field_name == "new_name" { Some(value.clone()) } else { None },
                new_description: if field_name == "new_description" { Some(value.clone()) } else { None },
                new_dp_url: if field_name == "new_dp_url" { Some(value) } else { None },
            };

            persist_group_message(&group_id, &msg).await;
        }

        // Return updated metadata
        if let Some(updated) = fetch_group_metadata(&group_id).await {
            HttpResponse::Ok().json(GroupResponse {
                group_id: updated.group_id,
                name: updated.group_name,
                description: updated.group_description, 
                display_picture_url: updated.group_dp_url,
                admins: updated.admins,
                members: updated.members,
                created_at: updated.created_at,
                updated_at: updated.updated_at,
            })
        } else {
            HttpResponse::InternalServerError().finish()
        }
    } else {
        HttpResponse::NotFound().finish()
    }
}

pub async fn get_user_profile(
    pubkey: web::Path<String>,
    req_pubkey: Option<String>, // From auth token/header
) -> HttpResponse {
    use crate::esmp::profile::{get_profile, generate_key};

    match get_profile(&pubkey).await {
        Some(mut profile) => {
            // If not the profile owner, return public view only
            let response = if Some(pubkey.to_string()) != req_pubkey {
                profile.to_public_view()
            } else {
                // For the owner, decrypt sensitive fields
                let key = generate_key(&profile.pubkey);
                if let Err(err) = profile.decrypt_sensitive_fields(&key) {
                    return HttpResponse::InternalServerError().json(serde_json::json!({
                        "error": format!("Failed to decrypt profile fields: {}", err)
                    }));
                }
                profile
            };

            HttpResponse::Ok().json(response)
        }
        None => HttpResponse::NotFound().finish()
    }
}

pub async fn update_user_profile(
    pubkey: web::Path<String>,
    req: web::Json<SignedProfileRequest>,
) -> HttpResponse {
    use crate::esmp::profile::{UserProfile, save_profile, generate_key, ProfileField, Visibility};
    
    // Verify the requester owns this profile
    if pubkey.as_str() != req.pubkey {
        return HttpResponse::Forbidden().json(serde_json::json!({
            "error": "Can only update your own profile"
        }));
    }

    // Validate request
    if let Err(validation_error) = req.request.validate() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": validation_error
        }));
    }

    // Verify signature 
    let request_bytes = serde_json::to_vec(&req.request).unwrap();
    if !verify_signature(&req.pubkey, &req.signature, &request_bytes) {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Invalid signature"
        }));
    }

    let now = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Get encryption key for sensitive fields
    let encryption_key = generate_key(&req.pubkey);

    // Build new profile with updated fields
    let mut profile = UserProfile {
        pubkey: req.pubkey.clone(),
        first_name: ProfileField {
            value: req.request.first_name.as_ref().and_then(|f| f.value.clone()),
            visibility: req.request.first_name.as_ref().and_then(|f| f.visibility.clone())
                .unwrap_or(Visibility::Private),
        },
        middle_name: ProfileField {
            value: req.request.middle_name.as_ref().and_then(|f| f.value.clone()),
            visibility: req.request.middle_name.as_ref().and_then(|f| f.visibility.clone())
                .unwrap_or(Visibility::Private),
        },
        last_name: ProfileField {
            value: req.request.last_name.as_ref().and_then(|f| f.value.clone()),
            visibility: req.request.last_name.as_ref().and_then(|f| f.visibility.clone())
                .unwrap_or(Visibility::Private),
        },
        display_picture: ProfileField {
            value: req.request.display_picture.as_ref().and_then(|f| f.value.clone()),
            visibility: req.request.display_picture.as_ref().and_then(|f| f.visibility.clone())
                .unwrap_or(Visibility::Private),
        },
        address: ProfileField {
            value: req.request.address.as_ref().and_then(|f| f.value.clone()).map(|s| s.into_bytes()),
            visibility: Visibility::Private, // Address is always private
        },
        updated_at: Some(now),
    };

    // Encrypt sensitive fields
    if let Err(err) = profile.encrypt_sensitive_fields(&encryption_key) {
        return HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("Failed to encrypt profile fields: {}", err)
        }));
    }    // Save profile and emit system message
    match save_profile(&profile).await {
        Ok(()) => {            // Emit profile update system message with actual changes
            let changes = ProfileChanges {
                first_name: req.request.first_name.as_ref().and_then(|f| f.value.clone()),
                middle_name: req.request.middle_name.as_ref().and_then(|f| f.value.clone()),
                last_name: req.request.last_name.as_ref().and_then(|f| f.value.clone()),
                display_picture: req.request.display_picture.as_ref().and_then(|f| f.value.clone()),
            };

            let system_msg = EsmpMessage {
                to: vec![], // Empty since this is just a notification
                cc: None,
                group_id: None,
                r#type: "system".to_string(),
                subtype: Some("profile_updated".to_string()),
                actor: Some(profile.pubkey.clone()),
                target: None,
                timestamp: Some(now),
                body: serde_json::json!({ "changes": changes }),
                signature: req.signature.clone(),
                sender_pubkey: req.pubkey.clone(),
                new_name: None,
                new_description: None,
                new_dp_url: None,
            };

            // Broadcast system message (implementation depends on your message handling)
            if let Err(err) = broadcast_system_message(&system_msg).await {
                log::warn!("Failed to broadcast profile update message: {}", err);
            }

            // Decrypt sensitive fields for response
            if let Err(err) = profile.decrypt_sensitive_fields(&encryption_key) {
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Failed to decrypt profile fields for response: {}", err)
                }));
            }

            HttpResponse::Ok().json(profile)
        }
        Err(err) => {
            let status = match &err {
                ProfileError::InvalidDisplayPictureUrl(_) |
                ProfileError::NameTooLong(_) |
                ProfileError::AddressTooLong |
                ProfileError::InvalidNameCharacter(_) => StatusCode::BAD_REQUEST,
                _ => StatusCode::INTERNAL_SERVER_ERROR
            };
            
            HttpResponse::build(status).json(serde_json::json!({
                "error": err.to_string()
            }))
        }
    }
}
