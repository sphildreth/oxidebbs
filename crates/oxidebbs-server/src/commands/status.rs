use serde_json::json;

use crate::{
    commands::doors,
    sysop_cli::{AppContext, CliResult, open_database, print_json},
};
use oxidebbs_db::list_active_sessions;
use oxidebbs_db::list_message_areas;

pub fn run_status(ctx: &AppContext) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    doors::sync_configured_doors(&db, &ctx.config)?;
    let active = list_active_sessions(db.db())?;
    let doors = doors::effective_doors(&db, &ctx.config)?;
    let enabled_doors = doors.iter().filter(|door| door.enabled).count();
    let message_areas = list_message_areas(db.db())?;
    let version = env!("CARGO_PKG_VERSION");

    if ctx.json {
        print_json(&json!({
            "board": ctx.config.board.name,
            "version": version,
            "database": ctx.config.database.path,
            "telnet": ctx.config.telnet.bind,
            "nodes": { "total": ctx.config.nodes.count, "active": active.len() },
            "doors": { "enabled": enabled_doors, "total": doors.len() },
            "messages": { "areas": message_areas.len() }
        }))?;
    } else {
        println!("OxideBBS Status");
        println!("Board:        {}", ctx.config.board.name);
        println!("Version:      {version}");
        println!("Database:     {}", ctx.config.database.path.display());
        println!("Telnet:       {}", ctx.config.telnet.bind);
        println!(
            "Nodes:        {} total, {} active",
            ctx.config.nodes.count,
            active.len()
        );
        println!("Doors:        {enabled_doors} enabled");
        println!("Messages:     {} areas", message_areas.len());
        println!("Uptime:       not available without a live control socket");
    }
    Ok(())
}
