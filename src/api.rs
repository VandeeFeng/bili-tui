use serde::Deserialize;

struct BilibiliClient {
    client: reqwest::Client,
    sessdata: String,
}

impl BilibiliClient {
    fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let sessdata = std::env::var("SESSDATA").unwrap_or_else(|_| "".to_string());
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .build()?;

        Ok(Self { client, sessdata })
    }

    async fn get_and_parse<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T, Box<dyn std::error::Error + Send + Sync>> {
        let response = self.client
            .get(url)
            .header("Cookie", format!("SESSDATA={}", self.sessdata))
            .send()
            .await?;

        let body_text = response.text().await?;
        serde_json::from_str(&body_text).map_err(|e| {
            format!("error decoding response body: {e}. Raw response: {body_text}")
                .into()
        })
    }

    async fn get_and_check<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T, Box<dyn std::error::Error + Send + Sync>> {
        let response = self.client
            .get(url)
            .header("Cookie", format!("SESSDATA={}", self.sessdata))
            .send()
            .await?;

        let status = response.status();
        let body_text = response.text().await?;

        if !status.is_success() {
            return Err(format!("HTTP error {}: {}. Response body: {}", status.as_u16(), status.canonical_reason().unwrap_or("Unknown"), body_text).into());
        }

        serde_json::from_str(&body_text).map_err(|e| {
            format!("error decoding response body: {e}. Raw response: {body_text}")
                .into()
        })
    }
}

#[derive(Deserialize, Debug)]
struct VideoInfoResponse {
    data: VideoInfo,
}

#[derive(Deserialize, Debug, Clone)]
pub struct VideoInfo {
    pub bvid: String,
    pub title: String,
    pub desc: String,
    pub owner: Owner,
    pub stat: Stat,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Owner {
    pub name: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Stat {
    pub view: u64,
    pub like: u64,
    #[allow(dead_code)]
    pub coin: u64,
    #[allow(dead_code)]
    pub favorite: u64,
    #[allow(dead_code)]
    pub share: u64,
}

#[derive(Deserialize, Debug)]
struct SearchResponse {
    data: SearchData,
}

#[derive(Deserialize, Debug)]
struct SearchData {
    result: Option<Vec<VideoResult>>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct VideoResult {
    pub r#type: String,
    pub author: String,
    pub bvid: String,
    #[serde(deserialize_with = "strip_em_tags")]
    pub title: String,
    pub description: String,
    pub play: serde_json::Value,
    pub like: u64,
    #[allow(dead_code)]
    pub video_review: u64,
    pub duration: String,
}

fn strip_em_tags<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Ok(s.replace("<em class=\"keyword\">", "").replace("</em>", ""))
}

pub async fn search(keyword: &str) -> Result<Vec<VideoResult>, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!(
        "https://api.bilibili.com/x/web-interface/search/type?search_type=video&keyword={}",
        keyword
    );
    let client = BilibiliClient::new()?;
    let response: SearchResponse = client.get_and_parse(&url).await?;

    let mut videos = vec![];
    if let Some(results) = response.data.result {
        videos = results
            .into_iter()
            .filter(|r| r.r#type == "video")
            .collect();
    }
    Ok(videos)
}

pub async fn get_video_info(bvid: &str) -> Result<VideoInfo, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!(
        "https://api.bilibili.com/x/web-interface/view?bvid={}",
        bvid
    );
    let client = BilibiliClient::new()?;
    let response: VideoInfoResponse = client.get_and_parse(&url).await?;
    Ok(response.data)
}

// Moments related structures and functions
#[derive(Deserialize, Debug, Clone)]
pub struct UserInfo {
    pub uid: u64,
    pub uname: String,
}


#[derive(Deserialize, Debug, Clone)]
pub struct AuthorItem {
    #[serde(rename = "user_profile")]
    pub user_profile: UserProfileMinimal,
}

#[derive(Deserialize, Debug, Clone)]
pub struct UserProfileMinimal {
    pub info: UserInfo,
}

#[derive(Deserialize, Debug)]
struct MomentsResponse {
    code: i32,
    message: String,
    data: MomentsData,
}

#[derive(Deserialize, Debug)]
struct MomentsData {
    items: Vec<AuthorItem>,
}




pub async fn get_moments() -> Result<Vec<AuthorItem>, Box<dyn std::error::Error + Send + Sync>> {
    let config = crate::config::FollowingConfig::load()?;

    if config.enable_custom_following {
        let mut authors = config.to_author_items();
        authors.retain(|author| !config.is_blacklisted(author.user_profile.info.uid));
        return Ok(authors);
    }

    let mut authors = load_moments_from_cache().unwrap_or_default();

    if authors.is_empty() {
        let url = "https://api.vc.bilibili.com/dynamic_svr/v1/dynamic_svr/w_dyn_uplist?teenagers_mode=0";
        let client = BilibiliClient::new()?;
        let api_response: MomentsResponse = client.get_and_check(url).await?;

        if api_response.code != 0 {
            return Err(format!("API returned error code {}: {}", api_response.code, api_response.message).into());
        }

        authors = api_response.data.items;
        save_moments_to_cache(&authors)?;
    }

    authors.retain(|author| !config.is_blacklisted(author.user_profile.info.uid));
    Ok(authors)
}

fn load_moments_from_cache() -> Option<Vec<AuthorItem>> {
    let config = crate::config::FollowingConfig::load().ok()?;
    if config.custom_authors.is_empty() {
        return None;
    }
    Some(config.to_author_items())
}

fn save_moments_to_cache(authors: &[AuthorItem]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut config = crate::config::FollowingConfig::load()?;
    config.update_from_api_data(authors);
    config.save()?;
    Ok(())
}

// Space API structures for user dynamics
#[derive(Deserialize, Debug, Clone)]
pub struct SpaceDynamicResponse {
    code: i32,
    message: String,
    data: SpaceDynamicData,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SpaceDynamicData {
    items: Vec<SpaceItem>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SpaceItem {
    modules: SpaceModules,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SpaceModules {
    #[serde(rename = "module_author")]
    module_author: ModuleAuthor,
    #[serde(rename = "module_dynamic")]
    module_dynamic: ModuleDynamic,
    #[serde(rename = "module_stat", default)]
    module_stat: Option<ModuleStat>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ModuleAuthor {
    name: String,
    #[serde(rename = "pub_ts")]
    pub_ts: u64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ModuleDynamic {
    desc: Option<ModuleDesc>,
    #[serde(default)]
    major: Option<ModuleMajor>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ModuleDesc {
    #[serde(default)]
    text: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ModuleMajor {
    #[serde(default)]
    archive: Option<ArchiveInfo>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ArchiveInfo {
    pub title: String,
    #[serde(rename = "duration_text")]
    pub duration_text: String,
    pub stat: ArchiveStat,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ArchiveStat {
    pub play: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ModuleStat {
    pub comment: StatItem,
    pub forward: StatItem,
    pub like: StatItem,
}

#[derive(Deserialize, Debug, Clone)]
pub struct StatItem {
    pub count: u64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AuthorDynamic {
    pub content: String,
    pub timestamp: u64,
    pub author_name: String,
    pub stats: Option<ModuleStat>,
    pub video_info: Option<ArchiveInfo>,
}

// Function to get dynamics for a specific user using space API
// Search for a video by title and return its bvid
pub async fn search_video_by_title(title: &str) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    let videos = search(title).await?;

    // Return the bvid of the first video that matches the title exactly
    for video in &videos {
        if video.title == title {
            return Ok(Some(video.bvid.clone()));
        }
    }

    // If no exact match, return the first video's bvid
    if let Some(first_video) = videos.first() {
        Ok(Some(first_video.bvid.clone()))
    } else {
        Ok(None)
    }
}

pub async fn get_user_dynamics(uid: u64) -> Result<Vec<AuthorDynamic>, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!(
        "https://api.bilibili.com/x/polymer/web-dynamic/v1/feed/space?host_mid={}",
        uid
    );
    let client = BilibiliClient::new()?;
    let api_response: SpaceDynamicResponse = client.get_and_check(&url).await?;

    if api_response.code != 0 {
        return Err(format!("API returned error code {}: {}", api_response.code, api_response.message).into());
    }

    let mut dynamics: Vec<AuthorDynamic> = api_response.data.items.into_iter().map(|item| {
        let author = &item.modules.module_author;
        let dynamic_content = &item.modules.module_dynamic;

        let content = dynamic_content.desc.as_ref()
            .map(|desc| desc.text.clone())
            .unwrap_or_default();

        let video_info = dynamic_content.major.as_ref()
            .and_then(|major| major.archive.clone());

        AuthorDynamic {
            content,
            timestamp: author.pub_ts,
            author_name: author.name.clone(),
            stats: item.modules.module_stat,
            video_info,
        }
    }).collect();

    // Sort dynamics by timestamp in descending order (newest first)
    dynamics.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    Ok(dynamics)
}

