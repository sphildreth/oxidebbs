use std::collections::BTreeMap;
use std::fs;

use clap::Subcommand;
use serde_json::{Value as JsonValue, json};

use oxidebbs_core::FtnAddress;
use oxidebbs_db::{
    NetworkAreaRecord, NetworkLinkRecord, NetworkNodelistRecord, NetworkPacketRecord,
    NetworkPollLogRecord, NetworkProfileRecord, find_network_link_by_key,
    find_network_nodelist_entry, find_network_profile_by_key, list_network_areas,
    list_network_links, list_network_messages, list_network_nodelist_entries, list_network_packets,
    list_network_poll_logs, list_network_profiles, replace_network_nodelist_entries,
};
use oxidebbs_ftn::{FtnNodelistEntry, apply_nodelist_diff, parse_nodelist};

use crate::sysop_cli::{
    AppContext, CliError, CliResult, audit, current_timestamp, emit_ok, generated_uuid,
    open_database, print_json,
};

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
    Import {
        file: String,
        #[arg(long)]
        network: Option<String>,
    },
    ApplyDiff {
        file: String,
        #[arg(long)]
        base: String,
        #[arg(long)]
        network: Option<String>,
    },
    Lookup {
        address: String,
        #[arg(long)]
        network: Option<String>,
    },
    List {
        #[arg(long)]
        network: Option<String>,
        #[arg(short, long, default_value_t = 50)]
        limit: usize,
    },
}

#[derive(Subcommand)]
pub enum NetAreasCommand {
    List {
        #[arg(long)]
        network: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum NetLinksCommand {
    List {
        #[arg(long)]
        network: Option<String>,
    },
}

pub fn run_net(command: NetCommand, ctx: &AppContext) -> CliResult<()> {
    match command {
        NetCommand::Toss { network } => unsupported_network_operation("toss", &network),
        NetCommand::Scan { network } => unsupported_network_operation("scan", &network),
        NetCommand::Poll { link } => unsupported_network_operation("poll", &link),
        NetCommand::Status { network } => run_net_status(ctx, &network),
        NetCommand::Nodelist { command } => run_nodelist(command, ctx),
        NetCommand::Areas { command } => match command {
            NetAreasCommand::List { network } => run_net_areas_list(ctx, network.as_deref()),
        },
        NetCommand::Links { command } => match command {
            NetLinksCommand::List { network } => run_net_links_list(ctx, network.as_deref()),
        },
        NetCommand::Logs { link } => run_net_logs(ctx, &link),
    }
}

fn unsupported_network_operation(operation: &str, target: &str) -> CliResult<()> {
    Err(CliError::Message(format!(
        "net {operation} for {target:?} requires the v1.2 FTN tosser/scanner/BinkP session engine, which is not implemented yet"
    )))
}

fn run_net_status(ctx: &AppContext, network: &str) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    let profile = require_network_profile(&db, network)?;
    let links = matching_links(&db, &profile.id)?;
    let areas = matching_areas(&db, &profile.id)?;
    let nodelist = matching_nodelist_entries(&db, &profile.id)?;
    let packets = matching_packets(&db, &profile.id)?;
    let messages = list_network_messages(db.db())?
        .into_iter()
        .filter(|message| message.network_id == profile.id)
        .collect::<Vec<_>>();

    if ctx.json {
        print_json(&json!({
            "network": network_profile_json(&profile),
            "counts": {
                "links": links.len(),
                "areas": areas.len(),
                "nodelist_entries": nodelist.len(),
                "packets": packets.len(),
                "messages": messages.len()
            },
            "packet_status": count_by_status(&packets)
        }))
    } else {
        println!(
            "{}\t{}\tadapter={}\taddress={}\tenabled={}",
            profile.key,
            profile.name,
            profile.adapter,
            profile_address(&profile),
            profile.enabled
        );
        println!(
            "links={}\tareas={}\tnodelist_entries={}\tpackets={}\tmessages={}",
            links.len(),
            areas.len(),
            nodelist.len(),
            packets.len(),
            messages.len()
        );
        for (status, count) in count_by_status(&packets) {
            println!("packet_status.{status}={count}");
        }
        Ok(())
    }
}

fn run_nodelist(command: NodelistCommand, ctx: &AppContext) -> CliResult<()> {
    match command {
        NodelistCommand::Import { file, network } => run_nodelist_import(ctx, &file, network),
        NodelistCommand::ApplyDiff {
            file,
            base,
            network,
        } => run_nodelist_apply_diff(ctx, &file, &base, network),
        NodelistCommand::Lookup { address, network } => run_nodelist_lookup(ctx, &address, network),
        NodelistCommand::List { network, limit } => {
            run_nodelist_list(ctx, network.as_deref(), limit)
        }
    }
}

fn run_nodelist_import(ctx: &AppContext, file: &str, network: Option<String>) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    let profile = resolve_network_profile(&db, network.as_deref())?;
    let bytes = fs::read(file)?;
    let contents = String::from_utf8_lossy(&bytes);
    let records = nodelist_records_from_contents(&db, &profile.id, &contents)?;

    replace_network_nodelist_entries(db.db(), &profile.id, &records)?;
    audit(
        &db,
        "network:nodelist:import",
        None,
        None,
        &format!(
            "imported {} nodelist entries for network {} from {}",
            records.len(),
            profile.key,
            file
        ),
    )?;
    emit_ok(
        ctx.json,
        "nodelist imported",
        json!({"network": profile.key, "entries": records.len(), "file": file}),
    )
}

fn run_nodelist_apply_diff(
    ctx: &AppContext,
    file: &str,
    base: &str,
    network: Option<String>,
) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    let profile = resolve_network_profile(&db, network.as_deref())?;
    let base_bytes = fs::read(base)?;
    let diff_bytes = fs::read(file)?;
    let base_contents = String::from_utf8_lossy(&base_bytes);
    let diff_contents = String::from_utf8_lossy(&diff_bytes);
    let updated_contents = apply_nodelist_diff(&base_contents, &diff_contents)
        .map_err(|error| CliError::Message(error.to_string()))?;
    let records = nodelist_records_from_contents(&db, &profile.id, &updated_contents)?;

    replace_network_nodelist_entries(db.db(), &profile.id, &records)?;
    audit(
        &db,
        "network:nodelist:apply-diff",
        None,
        None,
        &format!(
            "applied nodelist diff {} to base {} for network {} and imported {} entries",
            file,
            base,
            profile.key,
            records.len()
        ),
    )?;
    emit_ok(
        ctx.json,
        "nodelist diff applied",
        json!({"network": profile.key, "entries": records.len(), "file": file, "base": base}),
    )
}

fn nodelist_records_from_contents(
    db: &oxidebbs_db::OxideDb,
    network_id: &str,
    contents: &str,
) -> CliResult<Vec<NetworkNodelistRecord>> {
    let parsed = parse_nodelist(contents).map_err(|error| CliError::Message(error.to_string()))?;
    nodelist_records_from_entries(db, network_id, parsed)
}

fn nodelist_records_from_entries(
    db: &oxidebbs_db::OxideDb,
    network_id: &str,
    entries: Vec<FtnNodelistEntry>,
) -> CliResult<Vec<NetworkNodelistRecord>> {
    let imported_at = current_timestamp(db)?;
    let mut records = Vec::with_capacity(entries.len());
    for entry in entries {
        records.push(NetworkNodelistRecord {
            id: generated_uuid(db)?,
            network_id: network_id.to_string(),
            zone: i64::from(entry.address.zone),
            net: i64::from(entry.address.net),
            node: i64::from(entry.address.node),
            point: i64::from(entry.address.point.unwrap_or(0)),
            parsed_name: entry.name,
            raw_entry: entry.raw_entry,
            updated_at: imported_at.clone(),
        });
    }
    Ok(records)
}

fn run_nodelist_lookup(ctx: &AppContext, address: &str, network: Option<String>) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    let profile = resolve_network_profile(&db, network.as_deref())?;
    let address = address
        .parse::<FtnAddress>()
        .map_err(|error| CliError::Message(error.to_string()))?;
    let entry = find_network_nodelist_entry(
        db.db(),
        &profile.id,
        i64::from(address.zone),
        i64::from(address.net),
        i64::from(address.node),
        i64::from(address.point.unwrap_or(0)),
    )?
    .ok_or_else(|| {
        CliError::Message(format!(
            "nodelist entry {address} was not found for network {}",
            profile.key
        ))
    })?;

    if ctx.json {
        print_json(&json!({"network": profile.key, "entry": nodelist_entry_json(&entry)}))
    } else {
        print_nodelist_entry(&entry);
        Ok(())
    }
}

fn run_nodelist_list(ctx: &AppContext, network: Option<&str>, limit: usize) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    let profile = resolve_network_profile(&db, network)?;
    let entries: Vec<_> = matching_nodelist_entries(&db, &profile.id)?
        .into_iter()
        .take(limit)
        .collect();

    if ctx.json {
        print_json(
            &json!({"network": profile.key, "nodelist": entries.iter().map(nodelist_entry_json).collect::<Vec<_>>()}),
        )
    } else {
        for entry in entries {
            print_nodelist_entry(&entry);
        }
        Ok(())
    }
}

fn run_net_areas_list(ctx: &AppContext, network: Option<&str>) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    let areas = match network {
        Some(network) => {
            let profile = require_network_profile(&db, network)?;
            matching_areas(&db, &profile.id)?
        }
        None => list_network_areas(db.db())?,
    };

    if ctx.json {
        print_json(&json!({"areas": areas.iter().map(network_area_json).collect::<Vec<_>>()}))
    } else {
        for area in areas {
            println!(
                "{}\tnetwork={}\tlocal_area={}\tsubscribed={}\tread_only={}",
                area.area_tag, area.network_id, area.local_area_id, area.subscribed, area.read_only
            );
        }
        Ok(())
    }
}

fn run_net_links_list(ctx: &AppContext, network: Option<&str>) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    let links = match network {
        Some(network) => {
            let profile = require_network_profile(&db, network)?;
            matching_links(&db, &profile.id)?
        }
        None => list_network_links(db.db())?,
    };

    if ctx.json {
        print_json(&json!({"links": links.iter().map(network_link_json).collect::<Vec<_>>()}))
    } else {
        for link in links {
            println!(
                "{}\tnetwork={}\taddress={}\thost={}:{}\tcompression={}\tsecurity={}\tenabled={}",
                link.key,
                link.network_id,
                link.address,
                link.host,
                link.binkp_port,
                link.compression,
                link.transport_security,
                link.enabled
            );
        }
        Ok(())
    }
}

fn run_net_logs(ctx: &AppContext, link: &str) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    let link_record = require_network_link(&db, link)?;
    let logs: Vec<_> = list_network_poll_logs(db.db())?
        .into_iter()
        .filter(|log| log.link_id == link_record.id)
        .collect();

    if ctx.json {
        print_json(
            &json!({"link": network_link_json(&link_record), "poll_logs": logs.iter().map(poll_log_json).collect::<Vec<_>>()}),
        )
    } else {
        for log in logs {
            println!(
                "{}\t{}\tstatus={}\tin={}\tout={}\terror={}",
                log.started_at,
                log.direction,
                log.status,
                log.bytes_in,
                log.bytes_out,
                log.error_message.as_deref().unwrap_or("")
            );
        }
        Ok(())
    }
}

fn resolve_network_profile(
    db: &oxidebbs_db::OxideDb,
    key_or_id: Option<&str>,
) -> CliResult<NetworkProfileRecord> {
    match key_or_id {
        Some(value) => require_network_profile(db, value),
        None => {
            let profiles = list_network_profiles(db.db())?;
            let enabled: Vec<_> = profiles
                .iter()
                .filter(|profile| profile.enabled)
                .cloned()
                .collect();
            match (profiles.as_slice(), enabled.as_slice()) {
                ([only], _) => Ok(only.clone()),
                (_, [only]) => Ok(only.clone()),
                ([], _) => Err(CliError::Message(
                    "no network profiles exist; configure a [network.profiles] entry first"
                        .to_string(),
                )),
                _ => Err(CliError::Message(
                    "multiple network profiles exist; pass --network <key>".to_string(),
                )),
            }
        }
    }
}

fn require_network_profile(
    db: &oxidebbs_db::OxideDb,
    key_or_id: &str,
) -> CliResult<NetworkProfileRecord> {
    if let Some(profile) = find_network_profile_by_key(db.db(), key_or_id)? {
        return Ok(profile);
    }
    list_network_profiles(db.db())?
        .into_iter()
        .find(|profile| profile.id == key_or_id)
        .ok_or_else(|| CliError::Message(format!("network profile {key_or_id:?} was not found")))
}

fn require_network_link(
    db: &oxidebbs_db::OxideDb,
    key_or_id: &str,
) -> CliResult<NetworkLinkRecord> {
    if let Some(link) = find_network_link_by_key(db.db(), key_or_id)? {
        return Ok(link);
    }
    list_network_links(db.db())?
        .into_iter()
        .find(|link| link.id == key_or_id)
        .ok_or_else(|| CliError::Message(format!("network link {key_or_id:?} was not found")))
}

fn matching_links(
    db: &oxidebbs_db::OxideDb,
    network_id: &str,
) -> CliResult<Vec<NetworkLinkRecord>> {
    Ok(list_network_links(db.db())?
        .into_iter()
        .filter(|link| link.network_id == network_id)
        .collect())
}

fn matching_areas(
    db: &oxidebbs_db::OxideDb,
    network_id: &str,
) -> CliResult<Vec<NetworkAreaRecord>> {
    Ok(list_network_areas(db.db())?
        .into_iter()
        .filter(|area| area.network_id == network_id)
        .collect())
}

fn matching_nodelist_entries(
    db: &oxidebbs_db::OxideDb,
    network_id: &str,
) -> CliResult<Vec<NetworkNodelistRecord>> {
    Ok(list_network_nodelist_entries(db.db())?
        .into_iter()
        .filter(|entry| entry.network_id == network_id)
        .collect())
}

fn matching_packets(
    db: &oxidebbs_db::OxideDb,
    network_id: &str,
) -> CliResult<Vec<NetworkPacketRecord>> {
    Ok(list_network_packets(db.db())?
        .into_iter()
        .filter(|packet| packet.network_id == network_id)
        .collect())
}

fn count_by_status(packets: &[NetworkPacketRecord]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for packet in packets {
        *counts.entry(packet.status.clone()).or_insert(0) += 1;
    }
    counts
}

fn profile_address(profile: &NetworkProfileRecord) -> String {
    if profile.local_point > 0 {
        format!(
            "{}:{}/{}.{}",
            profile.local_zone, profile.local_net, profile.local_node, profile.local_point
        )
    } else {
        format!(
            "{}:{}/{}",
            profile.local_zone, profile.local_net, profile.local_node
        )
    }
}

fn address_for_nodelist_entry(entry: &NetworkNodelistRecord) -> String {
    if entry.point > 0 {
        format!(
            "{}:{}/{}.{}",
            entry.zone, entry.net, entry.node, entry.point
        )
    } else {
        format!("{}:{}/{}", entry.zone, entry.net, entry.node)
    }
}

fn print_nodelist_entry(entry: &NetworkNodelistRecord) {
    println!(
        "{}\t{}\tupdated={}",
        address_for_nodelist_entry(entry),
        entry.parsed_name.as_deref().unwrap_or(""),
        entry.updated_at
    );
}

fn network_profile_json(profile: &NetworkProfileRecord) -> JsonValue {
    json!({
        "id": profile.id,
        "key": profile.key,
        "name": profile.name,
        "adapter": profile.adapter,
        "local_address": profile_address(profile),
        "enabled": profile.enabled,
        "created_at": profile.created_at,
        "updated_at": profile.updated_at
    })
}

fn network_link_json(link: &NetworkLinkRecord) -> JsonValue {
    json!({
        "id": link.id,
        "key": link.key,
        "network_id": link.network_id,
        "address": link.address,
        "host": link.host,
        "binkp_port": link.binkp_port,
        "password": "[redacted]",
        "poll_schedule_minutes": link.poll_schedule_minutes,
        "compression": link.compression,
        "transport_security": link.transport_security,
        "enabled": link.enabled,
        "created_at": link.created_at,
        "updated_at": link.updated_at
    })
}

fn network_area_json(area: &NetworkAreaRecord) -> JsonValue {
    json!({
        "id": area.id,
        "network_id": area.network_id,
        "area_tag": area.area_tag,
        "local_area_id": area.local_area_id,
        "description": area.description,
        "read_only": area.read_only,
        "subscribed": area.subscribed,
        "created_at": area.created_at,
        "updated_at": area.updated_at
    })
}

fn nodelist_entry_json(entry: &NetworkNodelistRecord) -> JsonValue {
    json!({
        "id": entry.id,
        "network_id": entry.network_id,
        "address": address_for_nodelist_entry(entry),
        "zone": entry.zone,
        "net": entry.net,
        "node": entry.node,
        "point": entry.point,
        "parsed_name": entry.parsed_name,
        "raw_entry": entry.raw_entry,
        "updated_at": entry.updated_at
    })
}

fn poll_log_json(log: &NetworkPollLogRecord) -> JsonValue {
    json!({
        "id": log.id,
        "link_id": log.link_id,
        "started_at": log.started_at,
        "ended_at": log.ended_at,
        "direction": log.direction,
        "status": log.status,
        "bytes_in": log.bytes_in,
        "bytes_out": log.bytes_out,
        "packets_in": log.packets_in,
        "packets_out": log.packets_out,
        "error_message": log.error_message
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_link_json_redacts_password() {
        let link = NetworkLinkRecord {
            id: "link-id".to_string(),
            key: "boss".to_string(),
            network_id: "net-id".to_string(),
            address: "1:105/42".to_string(),
            host: "boss.example".to_string(),
            binkp_port: 24554,
            password: "secret".to_string(),
            poll_schedule_minutes: 30,
            compression: "zip".to_string(),
            transport_security: "tls_required".to_string(),
            enabled: true,
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
        };

        let value = network_link_json(&link);

        assert_eq!(value["password"], "[redacted]");
        assert!(!value.to_string().contains("secret"));
    }

    #[test]
    fn nodelist_entry_json_formats_point_addresses() {
        let entry = NetworkNodelistRecord {
            id: "node-id".to_string(),
            network_id: "net-id".to_string(),
            zone: 1,
            net: 105,
            node: 42,
            point: 7,
            parsed_name: Some("Point".to_string()),
            raw_entry: "Point,7,Point".to_string(),
            updated_at: "now".to_string(),
        };

        assert_eq!(nodelist_entry_json(&entry)["address"], "1:105/42.7");
    }
}
