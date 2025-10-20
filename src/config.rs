use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use crate::api::AuthorItem;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorInfo {
    pub uid: u64,
    pub username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct FollowingConfig {
    pub enable_custom_following: bool,
    pub custom_authors: Vec<AuthorInfo>,
    pub blacklist: Vec<AuthorInfo>,
    pub last_updated: u64,
}


impl FollowingConfig {
    pub fn load() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let config_path = get_config_path()?;

        if !config_path.exists() {
            let config = FollowingConfig::default();
            config.save()?;
            return Ok(config);
        }

        let content = fs::read_to_string(&config_path)?;
        let config: FollowingConfig = serde_json::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let config_path = get_config_path()?;

        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        fs::write(&config_path, content)?;
        Ok(())
    }

    pub fn add_custom_author(&mut self, uid: u64, username: String) {
        if let Some(pos) = self.custom_authors.iter().position(|author| author.uid == uid) {
            self.custom_authors[pos].username = username;
        } else {
            self.custom_authors.push(AuthorInfo { uid, username });
        }
        self.enable_custom_following = true;
        self.last_updated = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }

    pub fn remove_custom_author(&mut self, uid: u64) -> bool {
        let original_len = self.custom_authors.len();
        self.custom_authors.retain(|author| author.uid != uid);
        let was_removed = self.custom_authors.len() != original_len;

        if was_removed && self.custom_authors.is_empty() {
            self.enable_custom_following = false;
        }

        self.last_updated = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        was_removed
    }

    pub fn add_to_blacklist(&mut self, uid: u64, username: String) {
        if let Some(pos) = self.blacklist.iter().position(|author| author.uid == uid) {
            self.blacklist[pos].username = username;
        } else {
            self.blacklist.push(AuthorInfo { uid, username });
        }
        self.last_updated = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }

    pub fn remove_from_blacklist(&mut self, uid: u64) -> bool {
        let original_len = self.blacklist.len();
        self.blacklist.retain(|author| author.uid != uid);
        let was_removed = self.blacklist.len() != original_len;

        self.last_updated = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        was_removed
    }

    pub fn is_blacklisted(&self, uid: u64) -> bool {
        self.blacklist.iter().any(|author| author.uid == uid)
    }

    pub fn update_from_api_data(&mut self, authors: &[AuthorItem]) {
        self.custom_authors = authors.iter().map(|author| AuthorInfo {
            uid: author.user_profile.info.uid,
            username: author.user_profile.info.uname.clone(),
        }).collect();

        self.last_updated = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }

    pub fn to_author_items(&self) -> Vec<AuthorItem> {
        self.custom_authors.iter().map(|author| AuthorItem {
            user_profile: crate::api::UserProfileMinimal {
                info: crate::api::UserInfo {
                    uid: author.uid,
                    uname: author.username.clone(),
                },
            },
        }).collect()
    }
}

fn get_config_path() -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
    let config_dir = dirs::config_dir()
        .ok_or("Could not find config directory")?
        .join("bili-tui");

    Ok(config_dir.join("following.json"))
}