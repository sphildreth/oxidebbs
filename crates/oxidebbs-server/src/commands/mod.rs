//! Command handlers and command-specific types for the OxideBBS sysop CLI.

pub mod ansi;
pub mod audit;
pub mod config;
pub mod db;
pub mod doors;
pub mod files;
pub mod logs;
pub mod messages;
pub mod net;
pub mod nodes;
pub mod serve;
pub mod setup;
pub mod status;
pub mod sysop;
pub mod users;

pub(crate) use ansi::{AnsiCommand, run_ansi};
pub(crate) use audit::{AuditCommand, run_audit};
pub(crate) use config::{ConfigCommand, run_check, run_config, run_config_set};
pub(crate) use db::{DbCommand, run_db};
pub(crate) use doors::{DoorsCommand, run_doors};
pub(crate) use files::{FilesCommand, run_files};
pub(crate) use logs::{LogsCommand, run_logs};
pub(crate) use messages::{MessagesCommand, run_messages};
pub(crate) use net::{NetCommand, run_net};
pub(crate) use nodes::{NodesCommand, run_nodes};
pub(crate) use serve::{ServeArgs, run_serve};
pub(crate) use setup::{SetupArgs, run_setup_command};
pub(crate) use status::run_status;
pub(crate) use sysop::run_sysop_tui;
pub(crate) use users::{UsersCommand, run_users};
