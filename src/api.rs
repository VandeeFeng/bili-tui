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
    pub coin: u64,
    pub favorite: u64,
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
