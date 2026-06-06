mod admin_status;
mod admin_web;
mod binkp_listener;
mod commands;
mod config;
mod control;
mod door_session;
mod serve;
mod setup;
mod sysop_cli;
mod web_terminal;

#[tokio::main]
async fn main() {
    if let Err(error) = sysop_cli::run().await {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}
