use serde::{Deserialize, Serialize};
use std::fs;
use std::{
    collections::BTreeMap,
    sync::{OnceLock, RwLock},
    time::{SystemTime, UNIX_EPOCH},
};

static CLIENT: OnceLock<BilibiliClient> = OnceLock::new();
const API_ORIGIN: &str = "https://api.bilibili.com";
const SEARCH_API_PATH: &str = "/x/web-interface/wbi/search/type";
const MOMENTS_CACHE_TTL: u64 = 15 * 60;
const REFERER: &str = "https://www.bilibili.com/";
const MIXIN_KEY_ENC_TAB: [usize; 64] = [
    46, 47, 18, 2, 53, 8, 23, 32, 15, 50, 10, 31, 58, 3, 45, 35, 27, 43, 5, 49, 33, 9, 42, 19, 29,
    28, 14, 39, 12, 38, 41, 13, 37, 48, 7, 16, 24, 55, 40, 61, 26, 17, 0, 1, 60, 51, 30, 4, 22, 25,
    54, 21, 56, 59, 6, 63, 57, 62, 11, 36, 20, 34, 44, 52,
];

type ApiError = Box<dyn std::error::Error + Send + Sync>;
type Query = BTreeMap<String, String>;

#[derive(Debug)]
struct BilibiliClient {
    client: reqwest::Client,
    sessdata: String,
    device_cookies: RwLock<Option<String>>,
    wbi_keys: RwLock<Option<WbiKeys>>,
}

#[derive(Clone, Debug)]
struct WbiKeys {
    img_key: String,
    sub_key: String,
}

#[derive(Deserialize)]
struct ApiResponse<T> {
    code: i32,
    message: String,
    data: Option<T>,
}

#[derive(Deserialize)]
struct NavData {
    wbi_img: WbiImage,
}

#[derive(Deserialize)]
struct WbiImage {
    img_url: String,
    sub_url: String,
}

#[derive(Deserialize)]
struct FingerprintData {
    b_3: String,
    b_4: String,
}

impl<T> ApiResponse<T> {
    fn into_result(self) -> Result<T, ApiError> {
        if self.code != 0 {
            return Err(format!("Bilibili API error {}: {}", self.code, self.message).into());
        }
        self.data
            .ok_or_else(|| "Bilibili API returned no data".into())
    }
}

impl BilibiliClient {
    fn new() -> Result<Self, ApiError> {
        let sessdata = std::env::var("BILI_SESSDATA").unwrap_or_default();
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .build()?;

        Ok(Self {
            client,
            sessdata,
            device_cookies: RwLock::new(None),
            wbi_keys: RwLock::new(None),
        })
    }

    fn get() -> Result<&'static Self, ApiError> {
        if let Some(client) = CLIENT.get() {
            return Ok(client);
        }
        CLIENT
            .set(BilibiliClient::new()?)
            .map_err(|_| "Failed to initialize Bilibili client")?;
        CLIENT.get().ok_or_else(|| "Client not initialized".into())
    }

    async fn get_api<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        query: Query,
        wbi_sign: bool,
    ) -> Result<T, ApiError> {
        let mut response = self.request_api(path, query.clone(), wbi_sign).await?;
        if wbi_sign && response.code == -403 {
            *self.wbi_keys.write().map_err(|_| "WBI key lock poisoned")? = None;
            response = self.request_api(path, query, true).await?;
        }
        response.into_result()
    }

    async fn request_api<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        mut query: Query,
        wbi_sign: bool,
    ) -> Result<ApiResponse<T>, ApiError> {
        if wbi_sign {
            query = self.sign_query(query).await?;
        }
        let body = self.get_text(path, &query).await?;
        serde_json::from_str(&body).map_err(|error| {
            format!("error decoding response body: {error}. Raw response: {body}").into()
        })
    }

    async fn get_text(&self, path: &str, query: &Query) -> Result<String, ApiError> {
        // Fresh device cookies can cause public search to return a verification voucher instead of results.
        if path == SEARCH_API_PATH {
            return self.send_get(path, query, "").await;
        }
        let cookies = self.cookie_header().await?;
        self.send_get(path, query, &cookies).await
    }

    async fn send_get(&self, path: &str, query: &Query, cookies: &str) -> Result<String, ApiError> {
        let url = format!("{API_ORIGIN}{path}?{}", build_query(query));
        let mut request = self.client.get(url).header("Referer", REFERER);
        if !cookies.is_empty() {
            request = request.header("Cookie", cookies);
        }
        let response = request.send().await?;
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            return Err(format!("HTTP error {status}. Response body: {body}").into());
        }
        Ok(body)
    }

    async fn cookie_header(&self) -> Result<String, ApiError> {
        if let Some(cookies) = self
            .device_cookies
            .read()
            .map_err(|_| "Device cookie lock poisoned")?
            .clone()
        {
            return Ok(cookies);
        }
        let body = self
            .send_get(
                "/x/frontend/finger/spi",
                &Query::new(),
                &self.sessdata_cookie(),
            )
            .await?;
        let data: FingerprintData = serde_json::from_str::<ApiResponse<_>>(&body)?.into_result()?;
        let cookies = self.build_cookie_header(&data);
        *self
            .device_cookies
            .write()
            .map_err(|_| "Device cookie lock poisoned")? = Some(cookies.clone());
        Ok(cookies)
    }

    fn build_cookie_header(&self, fingerprint: &FingerprintData) -> String {
        let mut cookies = self.sessdata_cookie();
        if !cookies.is_empty() {
            cookies.push_str("; ");
        }
        cookies.push_str(&format!(
            "buvid3={}; buvid4={}",
            fingerprint.b_3, fingerprint.b_4
        ));
        cookies
    }

    fn sessdata_cookie(&self) -> String {
        if self.sessdata.is_empty() {
            String::new()
        } else {
            format!("SESSDATA={}", self.sessdata)
        }
    }

    async fn sign_query(&self, mut query: Query) -> Result<Query, ApiError> {
        let keys = self.wbi_keys().await?;
        let wts = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        query.insert("wts".into(), wts.to_string());
        let mixin_key = generate_mixin_key(&keys);
        let w_rid = format!(
            "{:x}",
            md5::compute(format!("{}{}", build_query(&query), mixin_key))
        );
        query.insert("w_rid".into(), w_rid);
        Ok(query)
    }

    async fn wbi_keys(&self) -> Result<WbiKeys, ApiError> {
        if let Some(keys) = self
            .wbi_keys
            .read()
            .map_err(|_| "WBI key lock poisoned")?
            .clone()
        {
            return Ok(keys);
        }
        let body = self.get_text("/x/web-interface/nav", &Query::new()).await?;
        let response: ApiResponse<NavData> = serde_json::from_str(&body)?;
        let image = response
            .data
            .ok_or_else(|| format!("WBI keys unavailable: {}", response.message))?
            .wbi_img;
        let keys = WbiKeys {
            img_key: extract_wbi_key(&image.img_url)?,
            sub_key: extract_wbi_key(&image.sub_url)?,
        };
        *self.wbi_keys.write().map_err(|_| "WBI key lock poisoned")? = Some(keys.clone());
        Ok(keys)
    }
}

fn extract_wbi_key(url: &str) -> Result<String, ApiError> {
    url.rsplit('/')
        .next()
        .and_then(|file| file.strip_suffix(".png"))
        .map(str::to_owned)
        .ok_or_else(|| format!("Invalid WBI key URL: {url}").into())
}

fn generate_mixin_key(keys: &WbiKeys) -> String {
    let raw_key = format!("{}{}", keys.img_key, keys.sub_key);
    MIXIN_KEY_ENC_TAB
        .iter()
        .take(32)
        .filter_map(|index| raw_key.as_bytes().get(*index).copied())
        .map(char::from)
        .collect()
}

fn encode_wbi_value(value: &str) -> String {
    let filtered: String = value
        .chars()
        .filter(|char| !"!'()*".contains(*char))
        .collect();
    filtered.bytes().fold(String::new(), |mut encoded, byte| {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
        encoded
    })
}

fn build_query(query: &Query) -> String {
    query
        .iter()
        .filter(|(_, value)| !value.is_empty())
        .map(|(key, value)| format!("{}={}", encode_wbi_value(key), encode_wbi_value(value)))
        .collect::<Vec<_>>()
        .join("&")
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
struct SearchData {
    result: Option<Vec<VideoResult>>,
}

impl SearchData {
    fn into_videos(self) -> Result<Vec<VideoResult>, ApiError> {
        Ok(self
            .result
            .ok_or("Bilibili search returned no result list (verification may be required)")?
            .into_iter()
            .filter(|result| result.r#type == "video")
            .collect())
    }
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

impl VideoResult {
    pub fn play_count(&self) -> u64 {
        self.play
            .as_u64()
            .or_else(|| self.play.as_str()?.parse().ok())
            .unwrap_or_default()
    }
}

fn strip_em_tags<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Ok(s.replace("<em class=\"keyword\">", "").replace("</em>", ""))
}

pub async fn search(
    keyword: &str,
) -> Result<Vec<VideoResult>, Box<dyn std::error::Error + Send + Sync>> {
    let query = Query::from([
        ("search_type".into(), "video".into()),
        ("keyword".into(), keyword.into()),
        ("page".into(), "1".into()),
        ("page_size".into(), "20".into()),
    ]);
    let data: SearchData = BilibiliClient::get()?
        .get_api(SEARCH_API_PATH, query, true)
        .await?;

    data.into_videos()
}

pub async fn get_video_info(
    bvid: &str,
) -> Result<VideoInfo, Box<dyn std::error::Error + Send + Sync>> {
    let query = Query::from([("bvid".into(), bvid.into())]);
    BilibiliClient::get()?
        .get_api("/x/web-interface/view", query, false)
        .await
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

#[derive(Deserialize)]
struct MomentsPortalData {
    up_list: Option<MomentsUpList>,
}

#[derive(Deserialize)]
struct MomentsUpList {
    items: Vec<MomentsUpItem>,
}

#[derive(Deserialize)]
struct MomentsUpItem {
    mid: u64,
    uname: String,
}

pub async fn get_moments(
    force_refresh: bool,
) -> Result<Vec<AuthorItem>, Box<dyn std::error::Error + Send + Sync>> {
    let mut config = crate::config::FollowingConfig::load()?;

    if config.enable_custom_following {
        let mut authors = config.to_author_items();
        authors.retain(|author| !config.is_blacklisted(author.user_profile.info.uid));
        return Ok(authors);
    }

    let cached = config.to_cached_author_items();
    let cache_age = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .saturating_sub(config.cached_at);
    let mut authors = if !force_refresh && cache_age < MOMENTS_CACHE_TTL {
        cached.clone()
    } else {
        Vec::new()
    };

    if authors.is_empty() {
        let query = Query::from([
            ("up_list_more".into(), "1".into()),
            ("web_location".into(), "333.1365".into()),
        ]);
        let data: MomentsPortalData = match BilibiliClient::get()?
            .get_api("/x/polymer/web-dynamic/v1/portal", query, true)
            .await
        {
            Ok(data) => data,
            Err(_) if !force_refresh && !cached.is_empty() => {
                let mut authors = cached;
                authors.retain(|author| !config.is_blacklisted(author.user_profile.info.uid));
                return Ok(authors);
            }
            Err(error) => return Err(error),
        };

        authors = data
            .up_list
            .map(|up_list| up_list.items)
            .unwrap_or_default()
            .into_iter()
            .map(|author| AuthorItem {
                user_profile: UserProfileMinimal {
                    info: UserInfo {
                        uid: author.mid,
                        uname: author.uname,
                    },
                },
            })
            .collect();
        config.update_from_api_data(&authors);
        let _ = config.save();
    }

    authors.retain(|author| !config.is_blacklisted(author.user_profile.info.uid));
    Ok(authors)
}

// Space API structures for user dynamics
#[derive(Deserialize, Debug, Clone)]
struct SpaceDynamicData {
    #[serde(default)]
    items: Vec<SpaceItem>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SpaceItem {
    #[serde(default)]
    id_str: Option<String>,
    modules: SpaceModules,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SpaceModules {
    #[serde(rename = "module_author", default)]
    module_author: Option<ModuleAuthor>,
    #[serde(rename = "module_dynamic", default)]
    module_dynamic: Option<ModuleDynamic>,
    #[serde(rename = "module_stat", default)]
    module_stat: Option<ModuleStat>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ModuleAuthor {
    name: String,
    pub_ts: String,
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
    #[serde(default)]
    opus: Option<OpusInfo>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct OpusInfo {
    summary: Option<ModuleDesc>,
    #[serde(default)]
    pics: Vec<serde_json::Value>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ArchiveInfo {
    pub bvid: String,
    pub title: String,
    #[serde(rename = "duration_text")]
    pub duration_text: String,
    pub stat: ArchiveStat,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ArchiveStat {
    pub play: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ModuleStat {
    pub comment: StatItem,
    pub forward: StatItem,
    pub like: StatItem,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct StatItem {
    pub count: u64,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct AuthorDynamic {
    pub content: String,
    pub timestamp: u64,
    pub author_name: String,
    pub stats: Option<ModuleStat>,
    pub video_info: Option<ArchiveInfo>,
    #[serde(default)]
    pub has_images: bool,
    #[serde(default)]
    pub opus_id: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct CachedDynamics {
    saved_at: u64,
    items: Vec<AuthorDynamic>,
}

fn dynamics_cache_path(uid: u64) -> Option<std::path::PathBuf> {
    Some(
        crate::config::data_dir()?
            .join("dynamics")
            .join(format!("{uid}.json")),
    )
}

fn save_dynamics(uid: u64, items: &[AuthorDynamic]) -> Result<(), ApiError> {
    let path = dynamics_cache_path(uid).ok_or("Could not find cache directory")?;
    fs::create_dir_all(path.parent().ok_or("Invalid cache path")?)?;
    let saved_at = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    fs::write(
        path,
        serde_json::to_vec(&CachedDynamics {
            saved_at,
            items: items.to_vec(),
        })?,
    )?;
    Ok(())
}

fn parse_dynamic(item: SpaceItem) -> Option<AuthorDynamic> {
    let author = item.modules.module_author.as_ref()?;
    let dynamic = item.modules.module_dynamic.as_ref()?;
    let opus = dynamic.major.as_ref().and_then(|major| major.opus.as_ref());
    let content = dynamic
        .desc
        .as_ref()
        .or_else(|| opus.and_then(|opus| opus.summary.as_ref()))
        .map(|desc| desc.text.clone())
        .unwrap_or_default();
    let has_images = opus.is_some_and(|opus| !opus.pics.is_empty());
    Some(AuthorDynamic {
        content,
        timestamp: author.pub_ts.parse().unwrap_or_default(),
        author_name: author.name.clone(),
        stats: item.modules.module_stat,
        video_info: dynamic
            .major
            .as_ref()
            .and_then(|major| major.archive.clone()),
        has_images,
        opus_id: has_images.then_some(item.id_str).flatten(),
    })
}

pub async fn get_user_dynamics(
    uid: u64,
) -> Result<Vec<AuthorDynamic>, Box<dyn std::error::Error + Send + Sync>> {
    let cached: Option<CachedDynamics> = dynamics_cache_path(uid)
        .and_then(|path| fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok());
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    if let Some(cache) = &cached
        && !cache.items.is_empty()
        && now.saturating_sub(cache.saved_at) < MOMENTS_CACHE_TTL
    {
        return Ok(cache.items.clone());
    }
    let query = Query::from([
        ("host_mid".into(), uid.to_string()),
        ("features".into(), "itemOpusStyle".into()),
    ]);
    let data: SpaceDynamicData = match BilibiliClient::get()?
        .get_api("/x/polymer/web-dynamic/v1/feed/space", query, true)
        .await
    {
        Ok(data) => data,
        Err(_) if cached.as_ref().is_some_and(|cache| !cache.items.is_empty()) => {
            return Ok(cached.unwrap().items);
        }
        Err(error) => return Err(error),
    };

    let mut dynamics: Vec<AuthorDynamic> =
        data.items.into_iter().filter_map(parse_dynamic).collect();

    dynamics.sort_by_key(|dynamic| std::cmp::Reverse(dynamic.timestamp));
    if dynamics.is_empty() {
        return Ok(cached.map_or(dynamics, |cache| cache.items));
    }
    let _ = save_dynamics(uid, &dynamics);
    Ok(dynamics)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_wbi_signature_components() {
        let query = Query::from([
            ("search_type".into(), "video".into()),
            ("keyword".into(), "rust tui!".into()),
        ]);
        let keys = WbiKeys {
            img_key: "0123456789abcdef0123456789abcdef".into(),
            sub_key: "fedcba9876543210fedcba9876543210".into(),
        };

        assert_eq!(build_query(&query), "keyword=rust%20tui&search_type=video");
        assert_eq!(
            generate_mixin_key(&keys),
            "1022a87ffdaf532cb45ee953dce8c96d"
        );
    }

    #[test]
    fn rejects_search_verification_voucher() {
        let data: SearchData = serde_json::from_str(r#"{"v_voucher":"challenge"}"#).unwrap();
        assert!(
            data.into_videos()
                .unwrap_err()
                .to_string()
                .contains("verification")
        );
    }

    #[test]
    fn parses_space_dynamics() {
        let feed: SpaceDynamicData = serde_json::from_str(
            r#"{"items":[
                {"modules":{"module_dynamic":{"desc":{"text":"ignored"}}}},
                {"id_str":"123","modules":{"module_author":{"name":"author","pub_ts":"123"},
                    "module_dynamic":{"desc":null,"major":{"opus":{"summary":{"text":"caption"},"pics":[{}]}}}}},
                {"modules":{"module_author":{"name":"author","pub_ts":"124"},
                    "module_dynamic":{"desc":{"text":"video"},"major":{"archive":
                    {"bvid":"BV1example","title":"test","duration_text":"1:00","stat":{"play":"123"}}}}}}
            ]}"#,
        )
        .unwrap();
        let mut items = feed.items.into_iter();
        assert!(parse_dynamic(items.next().unwrap()).is_none());
        let opus = parse_dynamic(items.next().unwrap()).unwrap();
        assert_eq!(opus.content, "caption");
        assert!(opus.has_images);
        assert_eq!(opus.opus_id.as_deref(), Some("123"));
        let video = parse_dynamic(items.next().unwrap()).unwrap();
        assert_eq!(video.video_info.unwrap().bvid, "BV1example");
    }

    #[test]
    fn reads_play_count_from_search_result() {
        let mut video: VideoResult = serde_json::from_str(
            r#"{"type":"video","author":"tester","bvid":"BV1xx","title":"test","description":"","play":12345,"like":1,"video_review":0,"duration":"1:00"}"#,
        )
        .unwrap();

        assert_eq!(video.play_count(), 12345);
        video.play = serde_json::Value::String("67890".into());
        assert_eq!(video.play_count(), 67890);
    }
}
