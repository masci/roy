// Copyright 2025 Massimiliano Pippi
// SPDX-License-Identifier: MIT

use clap::Parser;
use roy::{run, Args};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let mut builder = env_logger::Builder::new();
    let filter = args.verbosity.log_level_filter();
    builder.filter_level(filter);
    builder.init();

    run(args).await
}
