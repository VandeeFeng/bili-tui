use crate::api::AuthorItem;
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FollowingConfig {
    #[serde(default)]
    pub enable_custom_following: bool,
    #[serde(default)]
    pub custom_authors: Vec<AuthorInfo>,
    #[serde(default)]
    pub cached_authors: Vec<AuthorInfo>,
    #[serde(default)]
    pub cached_at: u64,
    #[serde(default)]
    pub favorites: Vec<AuthorInfo>,
    #[serde(default)]
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

        if content.trim().is_empty() {
            let config = FollowingConfig::default();
            config.save()?;
            return Ok(config);
        }

        serde_json::from_str(&content)
            .map_err(|error| format!("Invalid JSON in config file: {error}").into())
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

    fn update_timestamp(&mut self) {
        self.last_updated = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
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
        self.update_timestamp();
    }

    pub fn remove_custom_author(&mut self, uid: u64) -> bool {
        let original_len = self.custom_authors.len();
        self.custom_authors.retain(|author| author.uid != uid);
        let was_removed = self.custom_authors.len() != original_len;

        if was_removed && self.custom_authors.is_empty() {
            self.enable_custom_following = false;
        }

        self.update_timestamp();
        was_removed
    }

    pub fn add_to_blacklist(&mut self, uid: u64, username: String) {
        if let Some(pos) = self.blacklist.iter().position(|author| author.uid == uid) {
            self.blacklist[pos].username = username;
        } else {
            self.blacklist.push(AuthorInfo { uid, username });
        }
        self.update_timestamp();
    }

    pub fn remove_from_blacklist(&mut self, uid: u64) -> bool {
        let original_len = self.blacklist.len();
        self.blacklist.retain(|author| author.uid != uid);
        let was_removed = self.blacklist.len() != original_len;

        self.update_timestamp();
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
        self.update_timestamp();
    }

    pub fn remove_favorite(&mut self, uid: u64) -> bool {
        let original_len = self.favorites.len();
        self.favorites.retain(|author| author.uid != uid);
        let was_removed = self.favorites.len() != original_len;

        self.update_timestamp();
        was_removed
    }

    pub fn is_favorite(&self, uid: u64) -> bool {
        self.favorites.iter().any(|author| author.uid == uid)
    }

    pub fn update_from_api_data(&mut self, authors: &[AuthorItem]) {
        self.cached_authors = authors
            .iter()
            .map(|author| AuthorInfo {
                uid: author.user_profile.info.uid,
                username: author.user_profile.info.uname.clone(),
            })
            .collect();

        self.update_timestamp();
        self.cached_at = self.last_updated;
    }

    pub fn to_cached_author_items(&self) -> Vec<AuthorItem> {
        self.cached_authors.iter().map(AuthorItem::from).collect()
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

        following_authors.retain(|author| !favorite_uids.contains(&author.user_profile.info.uid));

        favorite_authors
            .into_iter()
            .chain(following_authors)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_cache_does_not_replace_custom_authors() {
        let mut config = FollowingConfig::default();
        config.add_custom_author(1, "custom".into());
        config.update_from_api_data(&[AuthorItem::from(&AuthorInfo {
            uid: 2,
            username: "api".into(),
        })]);

        let restored: FollowingConfig =
            serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
        assert_eq!(restored.to_author_items()[0].user_profile.info.uid, 1);
        assert_eq!(
            restored.to_cached_author_items()[0].user_profile.info.uid,
            2
        );
    }
}

pub(crate) fn data_dir() -> Option<PathBuf> {
    Some(dirs::home_dir()?.join(".bili-tui"))
}

fn get_config_path() -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
    let config_dir = data_dir().ok_or("Could not find home directory")?;

    Ok(config_dir.join("following.json"))
}
