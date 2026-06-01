mod commands;
mod config;
mod serve;
mod setup;
mod sysop_cli;

#[tokio::main]
async fn main() {
    if let Err(error) = sysop_cli::run().await {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}
