use crate::api::AuthorItem;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};
use std::fs;
use std::path::PathBuf;

impl From<&AuthorInfo> for AuthorItem {
    fn from(info: &AuthorInfo) -> Self {
        AuthorItem {
            user_profile: crate::api::UserProfileMinimal {
                info: crate::api::UserInfo {
                    uid: info.uid,
                    uname: info.username.clone(),
                },
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorInfo {
    pub uid: u64,
    pub username: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct FollowingConfig {
    pub enable_custom_following: bool,
    #[serde(default)]
    pub custom_authors: Vec<AuthorInfo>,
    #[serde(default)]
    pub favorites: Vec<AuthorInfo>,
    #[serde(default)]
    pub blacklist: Vec<AuthorInfo>,
    pub last_updated: u64,
}

impl Serialize for FollowingConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("FollowingConfig", 5)?;
        state.serialize_field("enable_custom_following", &self.enable_custom_following)?;
        state.serialize_field("custom_authors", &self.custom_authors)?;
        state.serialize_field("favorites", &self.favorites)?;
        state.serialize_field("blacklist", &self.blacklist)?;
        state.serialize_field("last_updated", &self.last_updated)?;
        state.end()
    }
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

        // Handle empty or invalid JSON file
        if content.trim().is_empty() {
            let config = FollowingConfig::default();
            config.save()?;
            return Ok(config);
        }

        let config: FollowingConfig = serde_json::from_str(&content)
            .map_err(|e| format!("Invalid JSON in config file: {}. Creating new config.", e))?;

        // For backwards compatibility: if the loaded config doesn't have the expected structure
        // (e.g., missing favorites field), resave it to ensure all fields are present
        // This will automatically add missing fields with default values due to #[serde(default)]
        if !content.contains("\"favorites\"") {
            config.save()?;
        }

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
        if let Some(pos) = self
            .custom_authors
            .iter()
            .position(|author| author.uid == uid)
        {
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

    pub fn add_favorite(&mut self, uid: u64, username: String) {
        if let Some(pos) = self.favorites.iter().position(|author| author.uid == uid) {
            self.favorites[pos].username = username;
        } else {
            self.favorites.push(AuthorInfo { uid, username });
        }
        self.last_updated = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }

    pub fn remove_favorite(&mut self, uid: u64) -> bool {
        let original_len = self.favorites.len();
        self.favorites.retain(|author| author.uid != uid);
        let was_removed = self.favorites.len() != original_len;

        self.last_updated = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        was_removed
    }

    pub fn is_favorite(&self, uid: u64) -> bool {
        self.favorites.iter().any(|author| author.uid == uid)
    }

    pub fn update_from_api_data(&mut self, authors: &[AuthorItem]) {
        self.custom_authors = authors
            .iter()
            .map(|author| AuthorInfo {
                uid: author.user_profile.info.uid,
                username: author.user_profile.info.uname.clone(),
            })
            .collect();

        self.last_updated = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }

    pub fn to_author_items(&self) -> Vec<AuthorItem> {
        self.custom_authors.iter().map(AuthorItem::from).collect()
    }

    pub fn to_favorite_author_items(&self) -> Vec<AuthorItem> {
        self.favorites.iter().map(AuthorItem::from).collect()
    }

    pub fn merge_authors(&self, mut following_authors: Vec<AuthorItem>) -> Vec<AuthorItem> {
        let favorite_authors = self.to_favorite_author_items();
        let favorite_uids: std::collections::HashSet<u64> = favorite_authors
            .iter()
            .map(|author| author.user_profile.info.uid)
            .collect();

        // Remove favorites from following list to avoid duplicates
        following_authors.retain(|author| !favorite_uids.contains(&author.user_profile.info.uid));

        // Combine: favorites first, then following authors
        favorite_authors
            .into_iter()
            .chain(following_authors)
            .collect()
    }
}

fn get_config_path() -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
    let config_dir = dirs::config_dir()
        .ok_or("Could not find config directory")?
        .join("bili-tui");

    Ok(config_dir.join("following.json"))
}
