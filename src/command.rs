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

// Parser helpers

fn validate_args_len(args: &[&str], expected: usize, usage: &str) -> Result<(), String> {
    if args.len() != expected {
        return Err(usage.to_string());
    }
    Ok(())
}

fn validate_no_args(args: &[&str], usage: &str) -> Result<(), String> {
    if !args.is_empty() {
        return Err(usage.to_string());
    }
    Ok(())
}

fn parse_uid(arg: &str) -> Result<u64, String> {
    arg.parse().map_err(|_| "Invalid UID".to_string())
}

pub fn parse(input: &str) -> Result<Command, String> {
    let input = input.trim();
    let parts: Vec<&str> = input.split_whitespace().collect();
    let command = parts.first().ok_or("No command entered")?;
    let args = &parts[1..];

    match *command {
        "video" => {
            validate_args_len(args, 1, "Usage: video <url>")?;
            Ok(Command::PlayUrl(args[0].to_string()))
        }
        "video-info" => {
            validate_args_len(args, 1, "Usage: video-info <url_or_bvid>")?;
            Ok(Command::ShowVideoInfo(args[0].to_string()))
        }
        "moments" | "m" => {
            validate_no_args(args, "Usage: moments (or m)")?;
            Ok(Command::ShowMoments)
        }
        "favorite" | "f" => {
            validate_no_args(args, "Usage: favorite (or f)")?;
            Ok(Command::ShowFavorites)
        }
        "add" => {
            validate_args_len(args, 2, "Usage: add <uid> <username>")?;
            let uid = parse_uid(args[0])?;
            Ok(Command::AddAuthor(uid, args[1].to_string()))
        }
        "rm" => {
            validate_args_len(args, 1, "Usage: rm <uid>")?;
            let uid = parse_uid(args[0])?;
            Ok(Command::RemoveAuthor(uid))
        }
        "ban" => {
            validate_args_len(args, 2, "Usage: ban <uid> <username>")?;
            let uid = parse_uid(args[0])?;
            Ok(Command::BanAuthor(uid, args[1].to_string()))
        }
        "unban" => {
            validate_args_len(args, 1, "Usage: unban <uid>")?;
            let uid = parse_uid(args[0])?;
            Ok(Command::UnbanAuthor(uid))
        }
        "add_f" => {
            validate_args_len(args, 2, "Usage: add_f <uid> <username>")?;
            let uid = parse_uid(args[0])?;
            Ok(Command::FavoriteAuthor(uid, args[1].to_string()))
        }
        "rm_f" => {
            validate_args_len(args, 1, "Usage: rm_f <uid>")?;
            let uid = parse_uid(args[0])?;
            Ok(Command::UnfavoriteAuthor(uid))
        }
        "list" => {
            validate_no_args(args, "Usage: list")?;
            Ok(Command::ListAuthors)
        }
        "refresh" => {
            validate_no_args(args, "Usage: refresh")?;
            Ok(Command::RefreshAuthors)
        }
        "toggle-custom" => {
            validate_no_args(args, "Usage: toggle-custom")?;
            Ok(Command::ToggleCustom)
        }
        "help" => Ok(Command::Help),
        "q" => Ok(Command::Quit),
        _ => Err(format!("Unknown command: {}", command)),
    }
}

// Command execution helpers

fn add_message(app: &mut App, text: String, level: crate::app::MessageLevel) {
    app.add_message(text, level);
}

fn save_config(app: &mut App) -> Result<(), String> {
    app.following_config
        .save()
        .map_err(|e| format!("Failed to save config: {}", e))
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
            add_message(
                app,
                format!("Loaded {} dynamics", count),
                crate::app::MessageLevel::Success,
            );
        }
        Err(e) => {
            add_message(
                app,
                format!("Failed to load dynamics: {}", e),
                crate::app::MessageLevel::Error,
            );
            app.selected_author_dynamics = None;
        }
    }

    app.loading_dynamics = false;
}

// Individual command handlers

async fn handle_play_url(app: &mut App, url: String) -> Result<(), String> {
    app.launch_mpv(&url);
    Ok(())
}

async fn handle_show_video_info(app: &mut App, url_or_bvid: String) -> Result<(), String> {
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

async fn handle_show_moments(app: &mut App) -> Result<(), String> {
    add_message(
        app,
        "Loading moments...".to_string(),
        crate::app::MessageLevel::Info,
    );

    match api::get_moments(false).await {
        Ok(following_authors) => {
            let authors = app.following_config.merge_authors(following_authors);
            let count = authors.len();
            app.moments_data = Some(authors);
            app.set_active_page(crate::app::ActivePage::Moments);
            app.set_input_mode(crate::app::InputMode::Normal);
            app.set_focused_panel(crate::app::Focusable::MomentsAuthors);

            if !app.moments_data.as_ref().unwrap().is_empty() {
                app.selected_author.select(Some(0));
                fetch_first_author_dynamics(app).await;
            }
            add_message(
                app,
                format!("Moments loaded successfully: {} authors found", count),
                crate::app::MessageLevel::Success,
            );
            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Failed to load moments: {}", e);
            if error_msg.contains("SESSDATA") {
                add_message(
                    app,
                    "Error: SESSDATA environment variable not set. Please set it to access Bilibili moments.".to_string(),
                    crate::app::MessageLevel::Error,
                );
                add_message(
                    app,
                    "Get SESSDATA from browser's cookie for bilibili.com (Developer Tools → Application → Cookies)".to_string(),
                    crate::app::MessageLevel::Warning,
                );
            } else {
                add_message(app, error_msg.clone(), crate::app::MessageLevel::Error);
            }
            Err(error_msg)
        }
    }
}

async fn handle_show_favorites(app: &mut App) -> Result<(), String> {
    add_message(
        app,
        "Loading favorite authors...".to_string(),
        crate::app::MessageLevel::Info,
    );

    let favorite_authors = app.following_config.to_favorite_author_items();
    let count = favorite_authors.len();

    if count == 0 {
        add_message(
            app,
            "No favorite authors found. Use 'favorite-add <uid> <username>' to add favorites."
                .to_string(),
            crate::app::MessageLevel::Warning,
        );
        return Ok(());
    }

    app.moments_data = Some(favorite_authors);
    app.set_active_page(crate::app::ActivePage::Moments);
    app.set_input_mode(crate::app::InputMode::Normal);
    app.set_focused_panel(crate::app::Focusable::MomentsAuthors);

    if !app.moments_data.as_ref().unwrap().is_empty() {
        app.selected_author.select(Some(0));
        fetch_first_author_dynamics(app).await;
    }
    add_message(
        app,
        format!("Loaded {} favorite authors", count),
        crate::app::MessageLevel::Success,
    );
    Ok(())
}

async fn handle_add_author(app: &mut App, uid: u64, username: String) -> Result<(), String> {
    app.following_config
        .add_custom_author(uid, username.clone());
    save_config(app)?;
    add_message(
        app,
        format!("Added author: {} (UID: {})", username, uid),
        crate::app::MessageLevel::Success,
    );
    Ok(())
}

async fn handle_remove_author(app: &mut App, uid: u64) -> Result<(), String> {
    if app.following_config.remove_custom_author(uid) {
        save_config(app)?;
        add_message(
            app,
            format!("Removed author with UID: {}", uid),
            crate::app::MessageLevel::Success,
        );
    } else {
        add_message(
            app,
            format!("Author with UID {} not found", uid),
            crate::app::MessageLevel::Warning,
        );
    }
    Ok(())
}

async fn handle_ban_author(app: &mut App, uid: u64, username: String) -> Result<(), String> {
    app.following_config.add_to_blacklist(uid, username.clone());
    save_config(app)?;
    add_message(
        app,
        format!("Banned author: {} (UID: {})", username, uid),
        crate::app::MessageLevel::Success,
    );
    Ok(())
}

async fn handle_unban_author(app: &mut App, uid: u64) -> Result<(), String> {
    if app.following_config.remove_from_blacklist(uid) {
        save_config(app)?;
        add_message(
            app,
            format!("Unbanned author with UID: {}", uid),
            crate::app::MessageLevel::Success,
        );
    } else {
        add_message(
            app,
            format!("Author with UID {} not found in blacklist", uid),
            crate::app::MessageLevel::Warning,
        );
    }
    Ok(())
}

async fn handle_favorite_author(app: &mut App, uid: u64, username: String) -> Result<(), String> {
    app.following_config.add_favorite(uid, username.clone());
    save_config(app)?;
    add_message(
        app,
        format!("Added to favorites: {} (UID: {})", username, uid),
        crate::app::MessageLevel::Success,
    );
    Ok(())
}

async fn handle_unfavorite_author(app: &mut App, uid: u64) -> Result<(), String> {
    if app.following_config.remove_favorite(uid) {
        save_config(app)?;
        add_message(
            app,
            format!("Removed from favorites: UID {}", uid),
            crate::app::MessageLevel::Success,
        );
    } else {
        add_message(
            app,
            format!("Author with UID {} not found in favorites", uid),
            crate::app::MessageLevel::Warning,
        );
    }
    Ok(())
}

fn list_favorite_authors(app: &mut App) {
    if !app.following_config.favorites.is_empty() {
        add_message(
            app,
            "⭐ Favorite authors:".to_string(),
            crate::app::MessageLevel::Info,
        );
        let favorites = app.following_config.favorites.clone();
        for author in &favorites {
            add_message(
                app,
                format!("  - ⭐ {} (UID: {})", author.username, author.uid),
                crate::app::MessageLevel::Info,
            );
        }
    }
}

fn list_custom_authors(app: &mut App) {
    if !app.following_config.custom_authors.is_empty() {
        add_message(
            app,
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
            add_message(
                app,
                format!("  - {}{} (UID: {})", star, author.username, author.uid),
                crate::app::MessageLevel::Info,
            );
        }
    }
}

fn list_api_authors(app: &mut App) {
    if app.moments_data.is_some() {
        add_message(
            app,
            "Following authors:".to_string(),
            crate::app::MessageLevel::Info,
        );

        let favorite_uids: std::collections::HashSet<_> = app
            .following_config
            .favorites
            .iter()
            .map(|author| author.uid)
            .collect();

        let moments_data_clone = app.moments_data.clone();
        if let Some(moments_data) = moments_data_clone {
            for author in &moments_data {
                let star = if favorite_uids.contains(&author.user_profile.info.uid) {
                    "⭐ "
                } else {
                    ""
                };
                add_message(
                    app,
                    format!(
                        "  - {}{} (UID: {})",
                        star, author.user_profile.info.uname, author.user_profile.info.uid
                    ),
                    crate::app::MessageLevel::Info,
                );
            }
        }
    } else {
        add_message(
            app,
            "No cached author data. Use 'moments' command to load following authors.".to_string(),
            crate::app::MessageLevel::Warning,
        );
    }
}

fn list_blacklist(app: &mut App) {
    if !app.following_config.blacklist.is_empty() {
        add_message(
            app,
            "Blacklisted authors:".to_string(),
            crate::app::MessageLevel::Warning,
        );
        let blacklist = app.following_config.blacklist.clone();
        for author in &blacklist {
            add_message(
                app,
                format!("  - {} (UID: {})", author.username, author.uid),
                crate::app::MessageLevel::Warning,
            );
        }
    }
}

fn check_empty_data(app: &mut App) {
    if app.following_config.custom_authors.is_empty()
        && app.following_config.blacklist.is_empty()
        && app.following_config.favorites.is_empty()
        && app.moments_data.is_none()
    {
        add_message(
            app,
            "No author data available. Use 'moments' command to load following authors."
                .to_string(),
            crate::app::MessageLevel::Info,
        );
    }
}

async fn handle_list_authors(app: &mut App) -> Result<(), String> {
    let status = if app.following_config.enable_custom_following {
        "Custom following (ON)"
    } else {
        "API following (Custom OFF)"
    };
    add_message(
        app,
        format!("Following status: {}", status),
        crate::app::MessageLevel::Info,
    );

    list_favorite_authors(app);

    if app.following_config.enable_custom_following {
        list_custom_authors(app);
    } else {
        list_api_authors(app);
    }

    list_blacklist(app);
    check_empty_data(app);

    Ok(())
}

async fn handle_refresh_authors(app: &mut App) -> Result<(), String> {
    if app.following_config.enable_custom_following {
        return Err(
            "Cannot refresh authors while custom following is enabled. Use 'toggle-custom' to disable custom following first."
                .to_string(),
        );
    }

    add_message(
        app,
        "Refreshing authors from API...".to_string(),
        crate::app::MessageLevel::Info,
    );

    match api::get_moments(true).await {
        Ok(authors) => {
            let count = authors.len();
            add_message(
                app,
                format!("Refreshed {} authors from API", count),
                crate::app::MessageLevel::Success,
            );
            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Failed to refresh authors: {}", e);
            add_message(app, error_msg.clone(), crate::app::MessageLevel::Error);
            Err(error_msg)
        }
    }
}

async fn handle_toggle_custom(app: &mut App) -> Result<(), String> {
    app.following_config.enable_custom_following = !app.following_config.enable_custom_following;
    save_config(app)?;
    let status = if app.following_config.enable_custom_following {
        "Custom following enabled"
    } else {
        "Custom following disabled"
    };
    add_message(app, status.to_string(), crate::app::MessageLevel::Success);
    Ok(())
}

// Main command dispatcher

pub async fn execute(command: Command, app: &mut App) -> Result<(), String> {
    match command {
        Command::PlayUrl(url) => handle_play_url(app, url).await,
        Command::ShowVideoInfo(url_or_bvid) => handle_show_video_info(app, url_or_bvid).await,
        Command::ShowMoments => handle_show_moments(app).await,
        Command::ShowFavorites => handle_show_favorites(app).await,
        Command::AddAuthor(uid, username) => handle_add_author(app, uid, username).await,
        Command::RemoveAuthor(uid) => handle_remove_author(app, uid).await,
        Command::BanAuthor(uid, username) => handle_ban_author(app, uid, username).await,
        Command::UnbanAuthor(uid) => handle_unban_author(app, uid).await,
        Command::FavoriteAuthor(uid, username) => handle_favorite_author(app, uid, username).await,
        Command::UnfavoriteAuthor(uid) => handle_unfavorite_author(app, uid).await,
        Command::ListAuthors => handle_list_authors(app).await,
        Command::RefreshAuthors => handle_refresh_authors(app).await,
        Command::ToggleCustom => handle_toggle_custom(app).await,
        Command::Help => {
            app.overlays.help = true;
            Ok(())
        }
        Command::Quit => Ok(()),
    }
}
