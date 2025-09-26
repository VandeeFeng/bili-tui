use crate::app::{App, InputMode};
use crate::api;
use url::Url;

#[derive(Debug, PartialEq)]
pub enum Command {
    PlayUrl(String),
    ShowVideoInfo(String),
    Quit,
}

pub fn parse(input: &str) -> Result<Command, String> {
    let input = input.trim();
    if !input.starts_with(':') {
        return Err("Commands must start with ':'".to_string());
    }

    let parts: Vec<&str> = input[1..].split_whitespace().collect();
    let command = parts.get(0).ok_or("No command entered")?;
    let args = &parts[1..];

    match *command {
        "video" => {
            if args.len() != 1 {
                return Err("Usage: :video <url>".to_string());
            }
            Ok(Command::PlayUrl(args[0].to_string()))
        }
        "video-info" => {
            if args.len() != 1 {
                return Err("Usage: :video-info <url_or_bvid>".to_string());
            }
            Ok(Command::ShowVideoInfo(args[0].to_string()))
        }
        "q" => Ok(Command::Quit),
        _ => Err(format!("Unknown command: {}", command)),
    }
}

fn extract_bvid(input: &str) -> Option<String> {
    if input.starts_with("BV") {
        return Some(input.to_string());
    }
    if let Ok(url) = Url::parse(input) {
        if let Some(domain) = url.domain() {
            if domain.ends_with("bilibili.com") {
                if let Some(path_segments) = url.path_segments() {
                    for segment in path_segments {
                        if segment.starts_with("BV") {
                            return Some(segment.to_string());
                        }
                    }
                }
            }
        }
    }
    None
}

pub async fn execute(command: Command, app: &mut App) -> Result<(), String> {
    match command {
        Command::PlayUrl(url) => {
            std::process::Command::new("mpv")
                .arg(&url)
                .spawn()
                .map_err(|e| format!("Failed to play video: {}", e))?;
            app.last_error = Some(format!("Playing: {}", url));
            Ok(())
        }
        Command::ShowVideoInfo(url_or_bvid) => {
            if let Some(bvid) = extract_bvid(&url_or_bvid) {
                match api::get_video_info(&bvid).await {
                    Ok(info) => {
                        app.video_info = Some(info);
                        app.mode = InputMode::Detail;
                        Ok(())
                    }
                    Err(e) => Err(e.to_string()),
                }
            } else {
                Err("Invalid Bilibili URL or BVID".to_string())
            }
        }
        Command::Quit => {
            Ok(())
        }
    }
}
