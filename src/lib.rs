use axum::{routing::post, Router};
use clap::Parser;
use clap_verbosity_flag::Verbosity;
use std::net::{IpAddr, SocketAddr};

pub mod chat_completion;
pub mod server_state;
use server_state::ServerState;

#[derive(Parser, Clone)]
#[command(name = "roy")]
#[command(
    about = "A HTTP server compatible with the OpenAI platform format that simulates errors and rate limit data"
)]
pub struct Args {
    #[command(flatten)]
    pub verbosity: Verbosity,

    #[arg(long, help = "Port to listen on", default_value = "8000")]
    pub port: u16,

    #[arg(long, help = "Address to listen on", default_value = "127.0.0.1")]
    pub address: IpAddr,

    #[arg(
        long,
        help = "Length of response (fixed number or range like '10:100')",
        default_value = "250"
    )]
    pub response_length: Option<String>,

    #[arg(long, help = "HTTP error code to return")]
    pub error_code: Option<u16>,

    #[arg(long, help = "Error rate percentage (0-100)")]
    pub error_rate: Option<u32>,

    #[arg(
        long,
        help = "Maximum number of requests per reset period",
        default_value = "60"
    )]
    pub x_ratelimit_limit_requests: u32,

    #[arg(
        long,
        help = "Request rate limit reset time in seconds",
        default_value = "1"
    )]
    pub x_ratelimit_reset_requests: u64,

    #[arg(
        long,
        help = "Maximum number of tokens per reset period",
        default_value = "150000"
    )]
    pub x_ratelimit_limit_tokens: u32,

    #[arg(
        long,
        help = "Token rate limit reset time in minutes",
        default_value = "6"
    )]
    pub x_ratelimit_reset_tokens: u64,
}

pub async fn run(args: Args) -> anyhow::Result<()> {
    let state = ServerState::new(args.clone());

    let app = Router::new()
        .route(
            "/v1/chat/completions",
            post(chat_completion::chat_completions),
        )
        .with_state(state);

    let addr = SocketAddr::new(args.address, args.port);
    let listener = tokio::net::TcpListener::bind(addr).await?;

    log::info!("Roy server running on http://{}", addr);
    axum::serve(listener, app).await?;

    Ok(())
}
