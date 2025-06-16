use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum SystemMessageType {
    Joined,
    Left,
    Removed,
    AdminAssigned,
    AdminRevoked,
    GroupCreated,
    GroupRenamed,
    DescriptionUpdated,
    DpUpdated,
    ProfileUpdated,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProfileChanges {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub middle_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_picture: Option<String>,
}

impl ToString for SystemMessageType {
    fn to_string(&self) -> String {
        match self {
            SystemMessageType::Joined => "joined",
            SystemMessageType::Left => "left",
            SystemMessageType::Removed => "removed",
            SystemMessageType::AdminAssigned => "admin_assigned",
            SystemMessageType::AdminRevoked => "admin_revoked",
            SystemMessageType::GroupCreated => "group_created",
            SystemMessageType::GroupRenamed => "group_renamed",
            SystemMessageType::DescriptionUpdated => "description_updated",
            SystemMessageType::DpUpdated => "dp_updated",
            SystemMessageType::ProfileUpdated => "profile_updated",
        }.to_string()
    }
}

impl SystemMessageType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "joined" => Some(Self::Joined),
            "left" => Some(Self::Left),
            "removed" => Some(Self::Removed),
            "admin_assigned" => Some(Self::AdminAssigned),
            "admin_revoked" => Some(Self::AdminRevoked),
            "group_created" => Some(Self::GroupCreated),
            "group_renamed" => Some(Self::GroupRenamed),
            "description_updated" => Some(Self::DescriptionUpdated),
            "dp_updated" => Some(Self::DpUpdated),
            "profile_updated" => Some(Self::ProfileUpdated),
            _ => None,
        }
    }
}
