# bili-tui

A TUI client for Bilibili written in Rust, created as a practice project. It provides a simple terminal interface for searching and playing videos directly from a URL.

Inspired by: [Siriusmart/youtube-tui: An aesthetically pleasing YouTube TUI written in Rust](https://github.com/Siriusmart/youtube-tui)

## Features

- **Video Search**: Search for Bilibili videos directly within the application.
- **Direct Playback**: Play video links directly using `mpv` and `yt-dlp`.
- **Video Information**: View detailed information about a specific video.
- **Command-line Interface**: Operate the client with simple commands.

## Prerequisites

Ensure you have the following software installed on your system:

- **[mpv](https://mpv.io/)**: A powerful media player.
- **[yt-dlp](https://github.com/yt-dlp/yt-dlp)**: A video downloader used to resolve video streams.

## How to Run

1.  **Clone the repository:**
    ```bash
    git clone https://github.com/vandeefeng/bili-tui.git
    cd bili-tui
    ```

2.  **Run with Cargo:**
    ```bash
    cargo run
    ```

To get better search results and video quality, you can provide your Bilibili SESSDATA via the `SESSDATA` environment variable.

**How to get SESSDATA:**
1. Log in to bilibili.com in your browser
2. Open Developer Tools (F12) → Application tab → Cookies → https://bilibili.com
3. Find the `SESSDATA` cookie and copy its value
4. Set the environment variable:
```bash
export SESSDATA="your_sessdata_value_here"
```

The application reads this variable and adds the SESSDATA to its API requests in `src/api.rs`:
```rust
// src/api.rs
let sessdata = std::env::var("SESSDATA").unwrap_or_else(|_| "".to_string());
// ...
let response = client.get(&url).header("Cookie", format!("SESSDATA={}", sessdata)).send().await?;
```

**Security Warning**: Storing SESSDATA in environment variables can be a security risk on shared systems. Use with caution.

## Commands
Navigation with JK and enter.

When into the command area:

- `:video <url>`: Plays the specified Bilibili video URL.
- `:video-info <url_or_bvid>`: Displays detailed information about the video (title, uploader, description, etc.).
- :moments or m : get the moments of your followings.
- `:help`: Shows the help screen.
- `:q`: Quits the application.Or quit the enter.

Also a quick search with `/`  and quick moments with 'm'.
