use clap::Subcommand;

use crate::sysop_cli::{AppContext, CliResult};

#[derive(Subcommand)]
pub enum NetCommand {
    Toss {
        network: String,
    },
    Scan {
        network: String,
    },
    Poll {
        link: String,
    },
    Status {
        network: String,
    },
    Nodelist {
        #[command(subcommand)]
        command: NodelistCommand,
    },
    Areas {
        #[command(subcommand)]
        command: NetAreasCommand,
    },
    Links {
        #[command(subcommand)]
        command: NetLinksCommand,
    },
    Logs {
        link: String,
    },
}

#[derive(Subcommand)]
pub enum NodelistCommand {
    Import { file: String },
    Lookup { address: String },
}

#[derive(Subcommand)]
pub enum NetAreasCommand {
    List,
}

#[derive(Subcommand)]
pub enum NetLinksCommand {
    List,
}

pub fn run_net(command: NetCommand, _ctx: &AppContext) -> CliResult<()> {
    match command {
        NetCommand::Toss { network } => {
            println!("FTN network operations are being implemented");
            println!("Would toss mail for network: {network}");
        }
        NetCommand::Scan { network } => {
            println!("FTN network operations are being implemented");
            println!("Would scan for mail on network: {network}");
        }
        NetCommand::Poll { link } => {
            println!("FTN network operations are being implemented");
            println!("Would poll link: {link}");
        }
        NetCommand::Status { network } => {
            println!("FTN network operations are being implemented");
            println!("Would show status for network: {network}");
        }
        NetCommand::Nodelist { command } => match command {
            NodelistCommand::Import { file } => {
                println!("FTN network operations are being implemented");
                println!("Would import nodelist from: {file}");
            }
            NodelistCommand::Lookup { address } => {
                println!("FTN network operations are being implemented");
                println!("Would lookup address: {address}");
            }
        },
        NetCommand::Areas { command } => match command {
            NetAreasCommand::List => {
                println!("FTN network operations are being implemented");
                println!("Would list echo areas");
            }
        },
        NetCommand::Links { command } => match command {
            NetLinksCommand::List => {
                println!("FTN network operations are being implemented");
                println!("Would list network links");
            }
        },
        NetCommand::Logs { link } => {
            println!("FTN network operations are being implemented");
            println!("Would show logs for link: {link}");
        }
    }
    Ok(())
}
