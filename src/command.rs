use crate::app::App;
use crate::api;
use url::Url;

#[derive(Debug, PartialEq)]
pub enum Command {
    PlayUrl(String),
    ShowVideoInfo(String),
    ShowMoments,
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
        "moments" | "m" => {
            if !args.is_empty() {
                return Err("Usage: moments (or m)".to_string());
            }
            Ok(Command::ShowMoments)
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

async fn fetch_first_author_dynamics(app: &mut App) {
    let uid = if let Some(data) = &app.moments_data {
        if let Some(first_author) = data.first() {
            first_author.user_profile.info.uid
        } else {
            return;
        }
    } else {
        return;
    };

    app.loading_dynamics = true;

    match api::get_user_dynamics(uid).await {
        Ok(dynamics) => {
            let count = dynamics.len();
            app.selected_author_dynamics = Some(dynamics);
            app.add_message(format!("Loaded {} dynamics", count), crate::app::MessageLevel::Success);
        }
        Err(e) => {
            app.add_message(format!("Failed to load dynamics: {}", e), crate::app::MessageLevel::Error);
            app.selected_author_dynamics = None;
        }
    }

    app.loading_dynamics = false;
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
                        app.set_active_page(crate::app::ActivePage::Detail);
                        app.set_input_mode(crate::app::InputMode::Normal);
                        Ok(())
                    }
                    Err(e) => Err(e.to_string()),
                }
            } else {
                Err("Invalid Bilibili URL or BVID".to_string())
            }
        }
        Command::ShowMoments => {
            app.add_message("Loading moments...".to_string(), crate::app::MessageLevel::Info);

            match api::get_moments().await {
                Ok(authors) => {
                    let count = authors.len();
                    app.moments_data = Some(authors);
                    app.set_active_page(crate::app::ActivePage::Moments);
                    app.set_input_mode(crate::app::InputMode::Normal);
                    app.set_focused_panel(crate::app::Focusable::MomentsAuthors);

                    if !app.moments_data.as_ref().unwrap().is_empty() {
                        app.selected_author.select(Some(0));
                        // Load dynamics for the first author
                        fetch_first_author_dynamics(app).await;
                    }
                    app.add_message(format!("Moments loaded successfully: {} authors found", count), crate::app::MessageLevel::Success);
                    Ok(())
                }
                Err(e) => {
                    let error_msg = format!("Failed to load moments: {}", e);
                    if error_msg.contains("SESSDATA") {
                        app.add_message("Error: SESSDATA environment variable not set. Please set it to access Bilibili moments.".to_string(), crate::app::MessageLevel::Error);
                        app.add_message("Get SESSDATA from browser's cookie for bilibili.com (Developer Tools → Application → Cookies)".to_string(), crate::app::MessageLevel::Warning);
                    } else {
                        app.add_message(error_msg.clone(), crate::app::MessageLevel::Error);
                    }
                    Err(error_msg)
                }
            }
        }
        Command::Help => {
            app.overlays.help = true;
            Ok(())
        }
        Command::Quit => {
            Ok(())
        }
    }
}
