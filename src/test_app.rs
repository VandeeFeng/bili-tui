// Test application to verify moments functionality without TUI
use crate::app::{App, InputMode};
use crate::api;

pub async fn test_moments_functionality() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Testing Moments Functionality ===");

    // Create a new app instance
    let mut app = App::new();

    // Test 1: Load moments data
    println!("\n1. Loading moments data...");
    match api::get_moments().await {
        Ok(authors) => {
            println!("✓ Successfully loaded {} authors", authors.len());
            app.moments_data = Some(authors);
            app.mode = InputMode::Moments;
            app.moments_active = true;

            if !app.moments_data.as_ref().unwrap().is_empty() {
                app.selected_author.select(Some(0));
                println!("✓ Selected first author");

                // Test 2: Load dynamics for first author
                println!("\n2. Loading dynamics for first author...");
                let first_author = app.moments_data.as_ref().unwrap().first().unwrap();
                let uid = first_author.user_profile.info.uid;

                app.loading_dynamics = true;
                match api::get_user_dynamics(uid).await {
                    Ok(dynamics) => {
                        println!("✓ Successfully loaded {} dynamics", dynamics.len());
                        app.selected_author_dynamics = Some(dynamics);
                        app.loading_dynamics = false;

                        // Test 3: Display sample data
                        println!("\n3. Sample data display:");
                        if let Some(dynamics) = &app.selected_author_dynamics {
                            for (i, dynamic) in dynamics.iter().take(3).enumerate() {
                                println!("Dynamic #{}", i + 1);
                                println!("  Author: {}", dynamic.author_name);
                                println!("  Content: {}", if dynamic.content.is_empty() { "[No text content]" } else { &dynamic.content[..dynamic.content.len().min(50)] });
                                if let Some(video) = &dynamic.video_info {
                                    println!("  Video: {}", video.title);
                                }
                                if let Some(stats) = &dynamic.stats {
                                    println!("  Stats: 👍{} 💬{} 🔄{}", stats.like.count, stats.comment.count, stats.forward.count);
                                }
                                println!();
                            }
                        }

                        println!("✓ All moments functionality working correctly!");
                        Ok(())
                    }
                    Err(e) => {
                        println!("✗ Failed to load dynamics: {}", e);
                        Err(e)
                    }
                }
            } else {
                println!("✗ No authors found");
                Err("No authors found".into())
            }
        }
        Err(e) => {
            println!("✗ Failed to load moments: {}", e);
            Err(e)
        }
    }
}