use audiofn::cli::{self, Cli};
use clap::Parser;

#[tokio::main]
async fn main() {
    audiofn::logging::init_logging();

    let cli = Cli::parse();
    let result = cli::run(cli).await;

    if let Err(err) = result {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
