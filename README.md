# bili-tui

A TUI client for Bilibili written in Rust, created as a practice project. It provides a simple terminal interface for searching and playing videos directly from a URL.

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

## Commands
Navigation with JK and enter.

When into the command area:

- `:video <url>`: Plays the specified Bilibili video URL.
- `:video-info <url_or_bvid>`: Displays detailed information about the video (title, uploader, description, etc.).
- `:help`: Shows the help screen.
- `:q`: Quits the application.Or quit the enter.

Also a quick search with `/` .
