use std::error::Error;

use clap::Parser;
use feroxyl::server;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser)]
#[command(name = "feroxyl")]
struct Args {
    /// Address to bind the server to
    #[arg(long, default_value = "localhost")]
    address: String,
    /// Port to listen on
    #[arg(short, long, default_value_t = 2010)]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "feroxyl=info,tower_http=info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();
    let bind_addr = format!("{}:{}", args.address, args.port);

    let app = server::create_app();

    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    tracing::info!("Listening on http://{}", bind_addr);
    axum::serve(listener, app).await?;
    Ok(())
}
