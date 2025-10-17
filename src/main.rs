mod api;
mod app;
mod command;
mod handler;
mod terminal;
mod ui;

use app::App;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let app = App::new();

    if let Err(err) = app.run().await {
        println!("{err:?}");
    }

    Ok(())
}
