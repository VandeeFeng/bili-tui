use serde::Deserialize;

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
    let cookie = std::env::var("BILI_COOKIE").unwrap_or_else(|_| "".to_string());
    let client = reqwest::Client::builder().user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36").build()?;
    let response = client.get(&url).header("Cookie", cookie).send().await?;

    let body_text = response.text().await?;
    let response = match serde_json::from_str::<SearchResponse>(&body_text) {
        Ok(parsed) => parsed,
        Err(e) => {
            return Err(format!(
                "error decoding response body: {e}. Raw response: {body_text}"
            )
            .into());
        }
    };

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
    let cookie = std::env::var("BILI_COOKIE").unwrap_or_else(|_| "".to_string());
    let client = reqwest::Client::builder().user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36").build()?;
    let response = client.get(&url).header("Cookie", cookie).send().await?;

    let body_text = response.text().await?;
    let response: VideoInfoResponse = match serde_json::from_str(&body_text) {
        Ok(parsed) => parsed,
        Err(e) => {
            return Err(format!(
                "error decoding response body: {e}. Raw response: {body_text}"
            )
            .into());
        }
    };

    Ok(response.data)
}

// Moments related structures and functions
#[derive(Deserialize, Debug, Clone)]
pub struct UserInfo {
    pub uid: u64,
    pub uname: String,
    pub face: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct OfficialVerify {
    #[serde(rename = "type")]
    pub verify_type: i32,
    pub desc: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct VipInfo {
    #[serde(rename = "avatar_subscript")]
    pub avatar_subscript: i32,
    #[serde(rename = "avatar_subscript_url", default)]
    pub avatar_subscript_url: String,
    #[serde(rename = "due_date")]
    pub due_date: u64,
    pub label: VipLabel,
    #[serde(rename = "nickname_color", default)]
    pub nickname_color: String,
    pub status: i32,
    #[serde(rename = "theme_type", default)]
    pub theme_type: i32,
    pub r#type: i32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct VipLabel {
    #[serde(rename = "bg_color", default)]
    pub bg_color: String,
    #[serde(rename = "bg_style", default)]
    pub bg_style: i32,
    #[serde(rename = "border_color", default)]
    pub border_color: String,
    #[serde(rename = "img_label_uri_hans", default)]
    pub img_label_uri_hans: String,
    #[serde(rename = "img_label_uri_hans_static", default)]
    pub img_label_uri_hans_static: String,
    #[serde(rename = "img_label_uri_hant", default)]
    pub img_label_uri_hant: String,
    #[serde(rename = "img_label_uri_hant_static", default)]
    pub img_label_uri_hant_static: String,
    #[serde(rename = "label_theme", default)]
    pub label_theme: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub text: String,
    #[serde(rename = "text_color", default)]
    pub text_color: String,
    #[serde(rename = "use_img_label", default)]
    pub use_img_label: bool,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Pendant {
    pub pid: u32,
    #[serde(default)]
    pub n_pid: u32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub image: String,
    #[serde(default)]
    pub image_enhance: String,
    #[serde(default)]
    pub image_enhance_frame: String,
    #[serde(default)]
    pub expire: u32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct LevelInfo {
    #[serde(rename = "current_level")]
    pub current_level: i32,
    #[serde(rename = "current_exp")]
    pub current_exp: u64,
    #[serde(rename = "next_exp")]
    pub next_exp: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Card {
    #[serde(rename = "official_verify")]
    pub official_verify: OfficialVerify,
}

#[derive(Deserialize, Debug, Clone)]
pub struct UserProfile {
    pub info: UserInfo,
    pub card: Card,
    pub vip: VipInfo,
    pub pendant: Pendant,
    pub rank: String,
    pub sign: String,
    #[serde(rename = "level_info")]
    pub level_info: LevelInfo,
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
    #[serde(rename = "button_statement", default)]
    button_statement: String,
    items: Vec<AuthorItem>,
    #[serde(default)]
    gt: i32,
}




pub async fn get_moments() -> Result<Vec<AuthorItem>, Box<dyn std::error::Error + Send + Sync>> {
    let url = "https://api.vc.bilibili.com/dynamic_svr/v1/dynamic_svr/w_dyn_uplist";

    let sessdata = match std::env::var("SESSDATA") {
        Ok(val) => {
            if val.is_empty() {
                return Err("SESSDATA environment variable is empty".into());
            }
            val
        }
        Err(e) => {
            return Err(format!("Failed to read SESSDATA environment variable: {}", e).into());
        }
    };

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .build()?;

    let response = client
        .get(url)
        .header("Cookie", format!("SESSDATA={}", sessdata))
        .query(&[("teenagers_mode", "0")])
        .send()
        .await?;

    let status = response.status();
    let body_text = response.text().await?;

    // Debug: write response to file for debugging
    if let Ok(mut file) = std::fs::File::create("/tmp/moments_debug.txt") {
        use std::io::Write;
        let _ = file.write_all(b"=== MOMENTS API DEBUG ===\n");
        let _ = file.write_all(format!("HTTP Status: {}\n", status).as_bytes());
        let _ = file.write_all(format!("Response Length: {}\n", body_text.len()).as_bytes());
        let _ = file.write_all(b"Response Body (first 1000 chars):\n");
        let _ = file.write_all(&body_text[..body_text.len().min(1000)].as_bytes());
        let _ = file.write_all(b"\n=== END DEBUG ===\n");
    }

    // First check HTTP status
    if !status.is_success() {
        return Err(format!("HTTP error {}: {}. Response body: {}", status.as_u16(), status.canonical_reason().unwrap_or("Unknown"), body_text).into());
    }

    // Then parse JSON response
    let api_response: MomentsResponse = match serde_json::from_str(&body_text) {
        Ok(parsed) => parsed,
        Err(e) => {
            // Write full response to file for debugging
            if let Ok(mut file) = std::fs::File::create("/tmp/moments_full_response.txt") {
                use std::io::Write;
                let _ = file.write_all(body_text.as_bytes());
            }
            return Err(format!(
                "error decoding moments response: {e}. Full response written to /tmp/moments_full_response.txt"
            )
            .into());
        }
    };

    if api_response.code != 0 {
        return Err(format!("API returned error code {}: {}", api_response.code, api_response.message).into());
    }

    Ok(api_response.data.items)
}

// Space API structures for user dynamics
#[derive(Deserialize, Debug, Clone)]
pub struct SpaceDynamicResponse {
    code: i32,
    message: String,
    ttl: i32,
    data: SpaceDynamicData,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SpaceDynamicData {
    #[serde(rename = "has_more")]
    has_more: bool,
    items: Vec<SpaceItem>,
    #[serde(default)]
    offset: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SpaceItem {
    #[serde(rename = "id_str")]
    id_str: String,
    #[serde(rename = "type")]
    item_type: String,
    basic: SpaceBasic,
    modules: SpaceModules,
    #[serde(default)]
    visible: bool,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SpaceBasic {
    #[serde(rename = "comment_id_str")]
    comment_id_str: String,
    #[serde(rename = "comment_type")]
    comment_type: i32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SpaceModules {
    #[serde(rename = "module_author")]
    module_author: ModuleAuthor,
    #[serde(rename = "module_dynamic")]
    module_dynamic: ModuleDynamic,
    #[serde(rename = "module_interaction", default)]
    module_interaction: Option<serde_json::Value>,
    #[serde(rename = "module_more", default)]
    module_more: Option<serde_json::Value>,
    #[serde(rename = "module_stat", default)]
    module_stat: Option<ModuleStat>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ModuleAuthor {
    face: String,
    #[serde(rename = "face_nft")]
    face_nft: bool,
    following: bool,
    #[serde(rename = "jump_url")]
    jump_url: String,
    label: String,
    mid: u64,
    name: String,
    #[serde(rename = "official_verify")]
    official_verify: OfficialVerify,
    pendant: Pendant,
    #[serde(rename = "pub_action")]
    pub_action: String,
    #[serde(rename = "pub_location_text", default)]
    pub_location_text: String,
    #[serde(rename = "pub_time")]
    pub_time: String,
    #[serde(rename = "pub_ts")]
    pub_ts: u64,
    r#type: String,
    vip: VipInfo,
    #[serde(default)]
    avatar: Option<serde_json::Value>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ModuleDynamic {
    #[serde(default)]
    additional: Option<serde_json::Value>,
    desc: Option<ModuleDesc>,
    #[serde(default)]
    major: Option<ModuleMajor>,
    #[serde(default)]
    topic: Option<serde_json::Value>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ModuleDesc {
    #[serde(default)]
    #[serde(rename = "rich_text_nodes")]
    rich_text_nodes: Vec<RichTextNode>,
    #[serde(default)]
    text: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct RichTextNode {
    #[serde(rename = "orig_text")]
    orig_text: String,
    text: String,
    r#type: String,
    #[serde(default)]
    #[serde(rename = "jump_url")]
    jump_url: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ModuleMajor {
    #[serde(default)]
    archive: Option<ArchiveInfo>,
    r#type: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ArchiveInfo {
    pub aid: String,
    pub bvid: String,
    pub title: String,
    pub cover: String,
    pub desc: String,
    #[serde(rename = "duration_text")]
    pub duration_text: String,
    #[serde(rename = "jump_url")]
    pub jump_url: String,
    pub stat: ArchiveStat,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ArchiveStat {
    #[serde(rename = "danmaku")]
    pub danmaku: String,
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
    #[serde(default)]
    pub forbidden: bool,
    #[serde(default)]
    pub status: bool,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AuthorDynamic {
    pub id_str: String,
    pub content: String,
    pub timestamp: u64,
    pub author_name: String,
    pub author_face: String,
    pub stats: Option<ModuleStat>,
    pub video_info: Option<ArchiveInfo>,
    pub dynamic_type: String,
}

// Function to get dynamics for a specific user using space API
pub async fn get_user_dynamics(uid: u64) -> Result<Vec<AuthorDynamic>, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!(
        "https://api.bilibili.com/x/polymer/web-dynamic/v1/feed/space?host_mid={}",
        uid
    );

    let sessdata = match std::env::var("SESSDATA") {
        Ok(val) => {
            if val.is_empty() {
                return Err("SESSDATA environment variable is empty".into());
            }
            val
        }
        Err(e) => {
            return Err(format!("Failed to read SESSDATA environment variable: {}", e).into());
        }
    };

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .build()?;

    let response = client
        .get(&url)
        .header("Cookie", format!("SESSDATA={}", sessdata))
        .send()
        .await?;

    let status = response.status();
    let body_text = response.text().await?;

    if !status.is_success() {
        return Err(format!("HTTP error {}: {}. Response body: {}", status.as_u16(), status.canonical_reason().unwrap_or("Unknown"), body_text).into());
    }

    let api_response: SpaceDynamicResponse = match serde_json::from_str(&body_text) {
        Ok(parsed) => parsed,
        Err(e) => {
            return Err(format!(
                "error decoding space response: {e}. Response body: {body_text}"
            )
            .into());
        }
    };

    if api_response.code != 0 {
        return Err(format!("API returned error code {}: {}", api_response.code, api_response.message).into());
    }

    let mut dynamics = vec![];
    for item in api_response.data.items {
        let author = &item.modules.module_author;
        let dynamic_content = &item.modules.module_dynamic;

        // Extract text content, fallback to empty string if not available
        let content = if let Some(desc) = &dynamic_content.desc {
            desc.text.clone()
        } else {
            String::new()
        };

        // Extract video info if available
        let video_info = if let Some(major) = &dynamic_content.major {
            major.archive.clone()
        } else {
            None
        };

        let dynamic = AuthorDynamic {
            id_str: item.id_str,
            content,
            timestamp: author.pub_ts,
            author_name: author.name.clone(),
            author_face: author.face.clone(),
            stats: item.modules.module_stat,
            video_info,
            dynamic_type: item.item_type,
        };

        dynamics.push(dynamic);
    }

    Ok(dynamics)
}

// Test function to debug the API call
pub async fn test_moments_api() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Testing moments API...");

    let sessdata = match std::env::var("SESSDATA") {
        Ok(val) => {
            println!("SESSDATA length: {}", val.len());
            if val.is_empty() {
                return Err("SESSDATA is empty".into());
            }
            val
        }
        Err(e) => {
            return Err(format!("Failed to read SESSDATA: {}", e).into());
        }
    };

    println!("Making request to API...");
    let url = "https://api.vc.bilibili.com/dynamic_svr/v1/dynamic_svr/w_dyn_uplist";
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .build()?;

    let response = client
        .get(url)
        .header("Cookie", format!("SESSDATA={}", sessdata))
        .query(&[("teenagers_mode", "0")])
        .send()
        .await?;

    let status = response.status();
    println!("HTTP Status: {}", status);

    let body_text = response.text().await?;
    println!("Response body length: {}", body_text.len());

    if !status.is_success() {
        return Err(format!("HTTP error: {}", status).into());
    }

    // Try to parse JSON
    let api_response: MomentsResponse = serde_json::from_str(&body_text)?;
    println!("API response code: {}", api_response.code);
    println!("Number of authors: {}", api_response.data.items.len());

    if api_response.code != 0 {
        return Err(format!("API error: {}", api_response.message).into());
    }

    Ok(())
}
