use crate::app::{App, InputMode};
use crate::api;
use url::Url;

#[derive(Debug, PartialEq)]
pub enum Command {
    PlayUrl(String),
    ShowVideoInfo(String),
    Help,
    Quit,
}

pub fn parse(input: &str) -> Result<Command, String> {
    let input = input.trim();
    let parts: Vec<&str> = input.split_whitespace().collect();
    let command = parts.first().ok_or("No command entered")?;
    let args = &parts[1..];

    match *command {
        "video" => {
            if args.len() != 1 {
                return Err("Usage: video <url>".to_string());
            }
            Ok(Command::PlayUrl(args[0].to_string()))
        }
        "video-info" => {
            if args.len() != 1 {
                return Err("Usage: video-info <url_or_bvid>".to_string());
            }
            Ok(Command::ShowVideoInfo(args[0].to_string()))
        }
        "help" => Ok(Command::Help),
        "q" => Ok(Command::Quit),
        _ => Err(format!("Unknown command: {}", command)),
    }
}

fn extract_bvid(input: &str) -> Option<String> {
    if input.starts_with("BV") {
        return Some(input.to_string());
    }
    if let Ok(url) = Url::parse(input)
        && let Some(domain) = url.domain()
            && domain.ends_with("bilibili.com")
                && let Some(path_segments) = url.path_segments() {
                    for segment in path_segments {
                        if segment.starts_with("BV") {
                            return Some(segment.to_string());
                        }
                    }
                }
    None
}

pub async fn execute(command: Command, app: &mut App) -> Result<(), String> {
    match command {
        Command::PlayUrl(url) => {
            std::process::Command::new("mpv")
                .arg("--no-terminal")
                .arg(&url)
                .spawn()
                .map_err(|e| format!("Failed to play video: {}", e))?;
            app.add_message(format!("Playing: {}", url), crate::app::MessageLevel::Info);
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
        Command::Help => {
            app.help_active = true;
            Ok(())
        }
        Command::Quit => {
            Ok(())
        }
    }
}
