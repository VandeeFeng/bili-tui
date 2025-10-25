use crate::api;
use crate::app::App;
use url::Url;

#[derive(Debug, PartialEq)]
pub enum Command {
    PlayUrl(String),
    ShowVideoInfo(String),
    ShowMoments,
    ShowFavorites,
    AddAuthor(u64, String),
    RemoveAuthor(u64),
    BanAuthor(u64, String),
    UnbanAuthor(u64),
    FavoriteAuthor(u64, String),
    UnfavoriteAuthor(u64),
    ListAuthors,
    RefreshAuthors,
    ToggleCustom,
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
        "favorite" | "f" => {
            if !args.is_empty() {
                return Err("Usage: favorite (or f)".to_string());
            }
            Ok(Command::ShowFavorites)
        }
        "add" => {
            if args.len() != 2 {
                return Err("Usage: add <uid> <username>".to_string());
            }
            let uid = args[0].parse().map_err(|_| "Invalid UID")?;
            Ok(Command::AddAuthor(uid, args[1].to_string()))
        }
        "rm" => {
            if args.len() != 1 {
                return Err("Usage: rm <uid>".to_string());
            }
            let uid = args[0].parse().map_err(|_| "Invalid UID")?;
            Ok(Command::RemoveAuthor(uid))
        }
        "ban" => {
            if args.len() != 2 {
                return Err("Usage: ban <uid> <username>".to_string());
            }
            let uid = args[0].parse().map_err(|_| "Invalid UID")?;
            Ok(Command::BanAuthor(uid, args[1].to_string()))
        }
        "unban" => {
            if args.len() != 1 {
                return Err("Usage: unban <uid>".to_string());
            }
            let uid = args[0].parse().map_err(|_| "Invalid UID")?;
            Ok(Command::UnbanAuthor(uid))
        }
        "add_f" => {
            if args.len() != 2 {
                return Err("Usage: add_f <uid> <username>".to_string());
            }
            let uid = args[0].parse().map_err(|_| "Invalid UID")?;
            Ok(Command::FavoriteAuthor(uid, args[1].to_string()))
        }
        "rm_f" => {
            if args.len() != 1 {
                return Err("Usage: rm_f <uid>".to_string());
            }
            let uid = args[0].parse().map_err(|_| "Invalid UID")?;
            Ok(Command::UnfavoriteAuthor(uid))
        }
        "list" => {
            if !args.is_empty() {
                return Err("Usage: list".to_string());
            }
            Ok(Command::ListAuthors)
        }
        "refresh" => {
            if !args.is_empty() {
                return Err("Usage: refresh".to_string());
            }
            Ok(Command::RefreshAuthors)
        }
        "toggle-custom" => {
            if !args.is_empty() {
                return Err("Usage: toggle-custom".to_string());
            }
            Ok(Command::ToggleCustom)
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
        && let Some(path_segments) = url.path_segments()
    {
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
            app.add_message(
                format!("Loaded {} dynamics", count),
                crate::app::MessageLevel::Success,
            );
        }
        Err(e) => {
            app.add_message(
                format!("Failed to load dynamics: {}", e),
                crate::app::MessageLevel::Error,
            );
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
            app.add_message(
                "Loading moments...".to_string(),
                crate::app::MessageLevel::Info,
            );

            match api::get_moments().await {
                Ok(following_authors) => {
                    // Merge following authors with standalone favorites
                    let authors = app.following_config.merge_authors(following_authors);

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
                    app.add_message(
                        format!("Moments loaded successfully: {} authors found", count),
                        crate::app::MessageLevel::Success,
                    );
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
        Command::ShowFavorites => {
            app.add_message(
                "Loading favorite authors...".to_string(),
                crate::app::MessageLevel::Info,
            );

            // Use favorites directly, no need to fetch from API
            let favorite_authors = app.following_config.to_favorite_author_items();
            let count = favorite_authors.len();

            if count == 0 {
                app.add_message("No favorite authors found. Use 'favorite-add <uid> <username>' to add favorites.".to_string(), crate::app::MessageLevel::Warning);
                return Ok(());
            }

            app.moments_data = Some(favorite_authors);
            app.set_active_page(crate::app::ActivePage::Moments);
            app.set_input_mode(crate::app::InputMode::Normal);
            app.set_focused_panel(crate::app::Focusable::MomentsAuthors);

            if !app.moments_data.as_ref().unwrap().is_empty() {
                app.selected_author.select(Some(0));
                // Load dynamics for the first author
                fetch_first_author_dynamics(app).await;
            }
            app.add_message(
                format!("Loaded {} favorite authors", count),
                crate::app::MessageLevel::Success,
            );
            Ok(())
        }
        Command::AddAuthor(uid, username) => {
            app.following_config
                .add_custom_author(uid, username.clone());
            if let Err(e) = app.following_config.save() {
                return Err(format!("Failed to save config: {}", e));
            }
            app.add_message(
                format!("Added author: {} (UID: {})", username, uid),
                crate::app::MessageLevel::Success,
            );
            Ok(())
        }
        Command::RemoveAuthor(uid) => {
            if app.following_config.remove_custom_author(uid) {
                if let Err(e) = app.following_config.save() {
                    return Err(format!("Failed to save config: {}", e));
                }
                app.add_message(
                    format!("Removed author with UID: {}", uid),
                    crate::app::MessageLevel::Success,
                );
            } else {
                app.add_message(
                    format!("Author with UID {} not found", uid),
                    crate::app::MessageLevel::Warning,
                );
            }
            Ok(())
        }
        Command::BanAuthor(uid, username) => {
            app.following_config.add_to_blacklist(uid, username.clone());
            if let Err(e) = app.following_config.save() {
                return Err(format!("Failed to save config: {}", e));
            }
            app.add_message(
                format!("Banned author: {} (UID: {})", username, uid),
                crate::app::MessageLevel::Success,
            );
            Ok(())
        }
        Command::UnbanAuthor(uid) => {
            if app.following_config.remove_from_blacklist(uid) {
                if let Err(e) = app.following_config.save() {
                    return Err(format!("Failed to save config: {}", e));
                }
                app.add_message(
                    format!("Unbanned author with UID: {}", uid),
                    crate::app::MessageLevel::Success,
                );
            } else {
                app.add_message(
                    format!("Author with UID {} not found in blacklist", uid),
                    crate::app::MessageLevel::Warning,
                );
            }
            Ok(())
        }
        Command::FavoriteAuthor(uid, username) => {
            app.following_config.add_favorite(uid, username.clone());
            if let Err(e) = app.following_config.save() {
                return Err(format!("Failed to save config: {}", e));
            }
            app.add_message(
                format!("Added to favorites: {} (UID: {})", username, uid),
                crate::app::MessageLevel::Success,
            );
            Ok(())
        }
        Command::UnfavoriteAuthor(uid) => {
            if app.following_config.remove_favorite(uid) {
                if let Err(e) = app.following_config.save() {
                    return Err(format!("Failed to save config: {}", e));
                }
                app.add_message(
                    format!("Removed from favorites: UID {}", uid),
                    crate::app::MessageLevel::Success,
                );
            } else {
                app.add_message(
                    format!("Author with UID {} not found in favorites", uid),
                    crate::app::MessageLevel::Warning,
                );
            }
            Ok(())
        }
        Command::ListAuthors => {
            let status = if app.following_config.enable_custom_following {
                "Custom following (ON)"
            } else {
                "API following (Custom OFF)"
            };
            app.add_message(
                format!("Following status: {}", status),
                crate::app::MessageLevel::Info,
            );

            if !app.following_config.favorites.is_empty() {
                app.add_message(
                    "⭐ Favorite authors:".to_string(),
                    crate::app::MessageLevel::Info,
                );
                let favorites = app.following_config.favorites.clone();
                for author in &favorites {
                    app.add_message(
                        format!("  - ⭐ {} (UID: {})", author.username, author.uid),
                        crate::app::MessageLevel::Info,
                    );
                }
            }

            if app.following_config.enable_custom_following {
                // Show custom authors if custom mode is enabled
                if !app.following_config.custom_authors.is_empty() {
                    app.add_message(
                        "Custom authors:".to_string(),
                        crate::app::MessageLevel::Info,
                    );
                    let authors = app.following_config.custom_authors.clone();
                    for author in &authors {
                        let star = if app.following_config.is_favorite(author.uid) {
                            "⭐ "
                        } else {
                            ""
                        };
                        app.add_message(
                            format!("  - {}{} (UID: {})", star, author.username, author.uid),
                            crate::app::MessageLevel::Info,
                        );
                    }
                }
            } else {
                // In API mode, show cached moments data if available
                if app.moments_data.is_some() {
                    app.add_message(
                        "Following authors:".to_string(),
                        crate::app::MessageLevel::Info,
                    );

                    // Get favorites for star display
                    let favorite_uids: std::collections::HashSet<_> = app
                        .following_config
                        .favorites
                        .iter()
                        .map(|author| author.uid)
                        .collect();

                    // Get a reference to moments_data and clone it to avoid borrowing issues
                    let moments_data_clone = app.moments_data.clone();
                    if let Some(moments_data) = moments_data_clone {
                        for author in &moments_data {
                            let star = if favorite_uids.contains(&author.user_profile.info.uid) {
                                "⭐ "
                            } else {
                                ""
                            };
                            app.add_message(
                                format!(
                                    "  - {}{} (UID: {})",
                                    star,
                                    author.user_profile.info.uname,
                                    author.user_profile.info.uid
                                ),
                                crate::app::MessageLevel::Info,
                            );
                        }
                    }
                } else {
                    app.add_message(
                        "No cached author data. Use 'moments' command to load following authors."
                            .to_string(),
                        crate::app::MessageLevel::Warning,
                    );
                }
            }

            if !app.following_config.blacklist.is_empty() {
                app.add_message(
                    "Blacklisted authors:".to_string(),
                    crate::app::MessageLevel::Warning,
                );
                let blacklist = app.following_config.blacklist.clone();
                for author in &blacklist {
                    app.add_message(
                        format!("  - {} (UID: {})", author.username, author.uid),
                        crate::app::MessageLevel::Warning,
                    );
                }
            }

            if app.following_config.custom_authors.is_empty()
                && app.following_config.blacklist.is_empty()
                && app.following_config.favorites.is_empty()
                && app.moments_data.is_none()
            {
                app.add_message(
                    "No author data available. Use 'moments' command to load following authors."
                        .to_string(),
                    crate::app::MessageLevel::Info,
                );
            }
            Ok(())
        }
        Command::RefreshAuthors => {
            if app.following_config.enable_custom_following {
                return Err("Cannot refresh authors while custom following is enabled. Use 'toggle-custom' to disable custom following first.".to_string());
            }

            app.add_message(
                "Refreshing authors from API...".to_string(),
                crate::app::MessageLevel::Info,
            );
            match api::get_moments().await {
                Ok(authors) => {
                    let count = authors.len();
                    app.add_message(
                        format!("Refreshed {} authors from API", count),
                        crate::app::MessageLevel::Success,
                    );
                    Ok(())
                }
                Err(e) => {
                    let error_msg = format!("Failed to refresh authors: {}", e);
                    app.add_message(error_msg.clone(), crate::app::MessageLevel::Error);
                    Err(error_msg)
                }
            }
        }
        Command::ToggleCustom => {
            app.following_config.enable_custom_following =
                !app.following_config.enable_custom_following;
            if let Err(e) = app.following_config.save() {
                return Err(format!("Failed to save config: {}", e));
            }
            let status = if app.following_config.enable_custom_following {
                "Custom following enabled"
            } else {
                "Custom following disabled"
            };
            app.add_message(status.to_string(), crate::app::MessageLevel::Success);
            Ok(())
        }
        Command::Help => {
            app.overlays.help = true;
            Ok(())
        }
        Command::Quit => Ok(()),
    }
}
