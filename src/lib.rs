// Copyright 2025 Massimiliano Pippi
// SPDX-License-Identifier: MIT

use axum::{http::Uri, routing::post, Router};
use clap::Parser;
use clap_verbosity_flag::Verbosity;
use colored::Colorize;
use std::net::{IpAddr, SocketAddr};

pub mod chat_completions;
pub mod models;
pub mod responses;
pub mod server_state;
use crate::server_state::ServerState;

#[derive(Parser, Clone)]
#[command(name = "roy")]
#[command(version = env!("CARGO_PKG_VERSION"))]
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
        help = "Maximum number of requests per minute",
        default_value = "500"
    )]
    pub rpm: u32,

    #[arg(
        long,
        help = "Maximum number of tokens per minute",
        default_value = "30000"
    )]
    pub tpm: u32,
}

pub async fn not_found(uri: Uri) -> (axum::http::StatusCode, String) {
    log::warn!("Path not found: {}", uri.path());
    (axum::http::StatusCode::NOT_FOUND, "Not Found".to_string())
}

pub async fn run(args: Args) -> anyhow::Result<()> {
    let state = ServerState::new(args.clone());

    let app = Router::new()
        .route(
            "/v1/chat/completions",
            post(chat_completions::chat_completions),
        )
        .route("/v1/responses", post(responses::responses))
        .fallback(not_found)
        .with_state(state);

    let addr = SocketAddr::new(args.address, args.port);
    let listener = tokio::net::TcpListener::bind(addr).await?;

    println!(
        "Roy server running on {}",
        format!("http://{}", addr).blue()
    );
    axum::serve(listener, app).await?;

    Ok(())
}
