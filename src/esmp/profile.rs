use crate::esmp::crypto::{encrypt_aes, decrypt_aes, generate_key};
use serde::{Serialize, Deserialize};
use tokio::fs::{OpenOptions, File};
use tokio::io::{AsyncWriteExt, AsyncBufReadExt, BufReader};
use std::time::{SystemTime, UNIX_EPOCH};
use common::url::Url;
use common::thiserror::Error;

#[derive(Debug, Error)]
pub enum ProfileError {
    #[error("Invalid display picture URL: {0}")]
    InvalidDisplayPictureUrl(String),
    #[error("Name too long: {0}")]
    NameTooLong(String),
    #[error("Address too long")]
    AddressTooLong,
    #[error("Invalid character in name: {0}")]
    InvalidNameCharacter(char),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Encryption error: {0}")]
    EncryptionError(String),
    #[error("Decryption error: {0}")]
    DecryptionError(String),
    #[error("Unauthorized access to private field")]
    UnauthorizedAccess,
}

const MAX_NAME_LENGTH: usize = 50;
const MAX_ADDRESS_LENGTH: usize = 200;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum Visibility {
    #[serde(rename = "public")]
    Public,
    #[serde(rename = "private")]
    Private,
}

impl Default for Visibility {
    fn default() -> Self {
        Visibility::Private
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProfileField<T> {
    pub value: Option<T>,
    pub visibility: Visibility,
}

impl<T> Default for ProfileField<T> {
    fn default() -> Self {
        Self {
            value: None,
            visibility: Visibility::default(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct UserProfile {
    pub pubkey: String,  // Ed25519 public key as base64
    pub first_name: ProfileField<String>,
    pub middle_name: ProfileField<String>,
    pub last_name: ProfileField<String>,
    pub display_picture: ProfileField<String>,
    pub address: ProfileField<Vec<u8>>,  // Encrypted when stored
    pub updated_at: Option<u64>,
}


impl UserProfile {
    pub fn new(pubkey: String) -> Self {
        Self {
            pubkey,
            first_name: ProfileField::default(),
            middle_name: ProfileField::default(),
            last_name: ProfileField::default(),
            display_picture: ProfileField::default(),
            address: ProfileField::default(),
            updated_at: Some(SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()),
        }
    }


    /// Set a profile field (except address)
    pub fn set_field(&mut self, field: &str, value: Option<String>, visibility: Option<Visibility>) {
        let vis = visibility.unwrap_or_default();
        match field {
            "first_name" => self.first_name = ProfileField { value, visibility: vis },
            "middle_name" => self.middle_name = ProfileField { value, visibility: vis },
            "last_name" => self.last_name = ProfileField { value, visibility: vis },
            "display_picture" => self.display_picture = ProfileField { value, visibility: vis },
            _ => {}
        }
    }

    /// Update the visibility of a specific field
    pub fn set_visibility(&mut self, field: &str, visibility: Visibility) {
        match field {
            "first_name" => self.first_name.visibility = visibility,
            "middle_name" => self.middle_name.visibility = visibility,
            "last_name" => self.last_name.visibility = visibility,
            "display_picture" => self.display_picture.visibility = visibility,
            "address" => self.address.visibility = visibility,
            _ => {}
        }
    }

    /// Set the address (encrypted, visibility can be set)
    pub fn set_address(&mut self, address: Option<String>, encryption_key: &[u8; 32], visibility: Option<Visibility>) -> Result<(), ProfileError> {
        let vis = visibility.unwrap_or_default();
        if let Some(addr) = address {
            if addr.len() > MAX_ADDRESS_LENGTH {
                return Err(ProfileError::AddressTooLong);
            }
            let encrypted = encrypt_aes(encryption_key, addr.as_bytes())?;
            self.address = ProfileField {
                value: Some(encrypted),
                visibility: vis,
            };
        } else {
            self.address = ProfileField { value: None, visibility: vis };
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), ProfileError> {
        // Validate display picture URL if present
        if let Some(url) = &self.display_picture.value {
            Url::parse(url)
                .map_err(|_| ProfileError::InvalidDisplayPictureUrl(url.clone()))?;
        }

        // Validate name fields
        for (name, field) in [
            ("first_name", &self.first_name),
            ("middle_name", &self.middle_name),
            ("last_name", &self.last_name),
        ] {
            if let Some(value) = &field.value {
                if value.len() > MAX_NAME_LENGTH {
                    return Err(ProfileError::NameTooLong(name.to_string()));
                }
                // Only allow letters, spaces, hyphens and apostrophes in names
                if let Some(invalid_char) = value.chars().find(|c| {
                    !c.is_alphabetic() && *c != ' ' && *c != '-' && *c != '\''
                }) {
                    return Err(ProfileError::InvalidNameCharacter(invalid_char));
                }
            }
        }

        // Validate address length (if decrypted)
        if let Some(addr_bytes) = &self.address.value {
            if addr_bytes.len() > MAX_ADDRESS_LENGTH {
                return Err(ProfileError::AddressTooLong);
            }
        }

        Ok(())
    }

    fn encrypt_sensitive_fields(&mut self, encryption_key: &[u8; 32]) -> Result<(), ProfileError> {
        // Encrypt address if present
        if let Some(address) = self.address.value.as_ref() {
            self.address.value = Some(encrypt_aes(encryption_key, address.as_bytes())?);
        }
        Ok(())
    }

    fn decrypt_sensitive_fields(&mut self, encryption_key: &[u8; 32]) -> Result<(), ProfileError> {
        // Decrypt address if present
        if let Some(encrypted_address) = self.address.value.as_ref() {
            let decrypted = decrypt_aes(encryption_key, encrypted_address)?;
            self.address.value = Some(String::from_utf8(decrypted)
                .map_err(|_| ProfileError::DecryptionError("Invalid UTF-8 in address".into()))?
                .into_bytes());
        }
        Ok(())
    }

    pub fn to_public_view(&self) -> Self {
        let mut public = self.clone();

        // Only include fields marked as public
        if self.first_name.visibility != Visibility::Public {
            public.first_name.value = None;
        }
        if self.middle_name.visibility != Visibility::Public {
            public.middle_name.value = None;
        }
        if self.last_name.visibility != Visibility::Public {
            public.last_name.value = None;
        }
        if self.display_picture.visibility != Visibility::Public {
            public.display_picture.value = None;
        }
        if self.address.visibility != Visibility::Public {
            public.address.value = None;
        }

        public
    }
}

pub async fn save_profile(profile: &UserProfile) -> Result<(), ProfileError> {
    // Validate profile before saving
    profile.validate()?;

    let profile_file = format!("user_profile_{}.json", profile.pubkey);
    let json = serde_json::to_string_pretty(profile)?;
    
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&profile_file)
        .await?;
        
    file.write_all(json.as_bytes()).await?;
    file.flush().await?;
    Ok(())
}

pub async fn get_profile(pubkey: &str) -> Option<UserProfile> {
    let profile_file = format!("user_profile_{}.json", pubkey);
    match tokio::fs::read_to_string(&profile_file).await {
        Ok(content) => serde_json::from_str(&content).ok(),
        Err(_) => None,
    }
}
