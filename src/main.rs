mod api;
mod app;
mod command;
mod handler;
mod terminal;
mod ui;
mod test_app;

use app::App;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Load environment variables from .env file
    dotenv::dotenv().ok();

    // Check for test argument
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && args[1] == "test" {
        println!("Running moments API test...");
        match api::test_moments_api().await {
            Ok(_) => println!("Test completed successfully!"),
            Err(e) => println!("Test failed: {}", e),
        }
        return Ok(());
    }

    if args.len() > 1 && args[1] == "dynamics-test" {
        println!("Testing user dynamics API...");
        // Test with the first author from moments (UID 7773004 from debug output)
        let test_uid = 7773004;
        println!("Testing dynamics for UID: {}", test_uid);

        match api::get_user_dynamics(test_uid).await {
            Ok(dynamics) => {
                println!("Successfully loaded {} dynamics!", dynamics.len());
                println!();

                for (i, dynamic) in dynamics.iter().take(3).enumerate() {
                    println!("=== Dynamic {} ===", i + 1);
                    println!("Author: {}", dynamic.author_name);
                    println!("Content: {}", dynamic.content);
                    println!("Timestamp: {}", dynamic.timestamp);

                    if let Some(video) = &dynamic.video_info {
                        println!("Video: {}", video.title);
                        println!("Duration: {}", video.duration_text);
                        println!("Plays: {}", video.stat.play);
                    }

                    if let Some(stats) = &dynamic.stats {
                        println!("Stats: 👍 {} 💬 {} 🔄 {}", stats.like.count, stats.comment.count, stats.forward.count);
                    }

                    println!();
                }

                if dynamics.len() > 3 {
                    println!("... and {} more dynamics", dynamics.len() - 3);
                }
            }
            Err(e) => {
                println!("Error: {}", e);
            }
        }
        return Ok(());
    }

    if args.len() > 1 && args[1] == "moments-full-test" {
        println!("Testing complete moments functionality...");
        match test_app::test_moments_functionality().await {
            Ok(_) => println!("✓ Full moments functionality test passed!"),
            Err(e) => println!("✗ Test failed: {}", e),
        }
        return Ok(());
    }

    let app = App::new();

    if let Err(err) = app.run().await {
        println!("{err:?}");
    }

    Ok(())
}
