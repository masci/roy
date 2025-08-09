use clap::Parser;
use roy::{run, Args};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    env_logger::Builder::new()
        .filter_level(args.verbosity.log_level_filter())
        .init();

    run(args).await
}
