use std::collections::BTreeMap;
use std::fs;

use clap::Subcommand;
use serde_json::{Value as JsonValue, json};

use oxidebbs_binkp::transport_security_plan;
use oxidebbs_core::FtnAddress;
use oxidebbs_db::{
    NetworkAreaRecord, NetworkLinkRecord, NetworkNodelistRecord, NetworkPacketRecord,
    NetworkPacketSummaryRecord, NetworkPollLogRecord, NetworkProfileRecord,
    NetworkSubscriptionRecord, find_network_area_by_tag_and_profile, find_network_link_by_key,
    find_network_nodelist_entry, find_network_packet_by_id, find_network_profile_by_key,
    insert_network_subscription, list_network_areas, list_network_links, list_network_messages,
    list_network_nodelist_entries, list_network_packets, list_network_poll_logs,
    list_network_profiles, list_network_subscriptions, mark_network_packet_quarantined,
    replace_network_nodelist_entries, requeue_network_packet, set_network_area_subscribed,
    set_network_subscription_status, summarize_network_packets,
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
        link: Option<String>,
        #[arg(long, default_value_t = false)]
        all: bool,
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
    Status {
        network: String,
    },
    Queue {
        link: String,
    },
    Packets {
        #[command(subcommand)]
        command: NetPacketsCommand,
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
        link: Option<String>,
        #[arg(short, long, default_value_t = 50)]
        limit: usize,
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
    Subscribe {
        area_tag: String,
        link: String,
        #[arg(long)]
        network: Option<String>,
    },
    Unsubscribe {
        area_tag: String,
        link: String,
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
    Show {
        link: String,
    },
}

#[derive(Subcommand)]
pub enum NetPacketsCommand {
    Summary {
        #[arg(long)]
        network: Option<String>,
    },
    Show {
        packet_id: String,
    },
    Retry {
        packet_id: String,
    },
    MarkQuarantined {
        packet_id: String,
        #[arg(long)]
        reason: String,
    },
    Inbound {
        #[arg(long)]
        network: Option<String>,
        #[arg(short, long, default_value_t = 50)]
        limit: usize,
    },
    Outbound {
        #[arg(long)]
        network: Option<String>,
        #[arg(short, long, default_value_t = 50)]
        limit: usize,
    },
    Quarantine {
        #[arg(long)]
        network: Option<String>,
        #[arg(short, long, default_value_t = 50)]
        limit: usize,
    },
}

pub fn run_net(command: NetCommand, ctx: &AppContext) -> CliResult<()> {
    match command {
        NetCommand::Toss { network } => unsupported_network_operation("toss", &network),
        NetCommand::Scan { network } => unsupported_network_operation("scan", &network),
        NetCommand::Poll { link, all, dry_run } => run_net_poll(ctx, link, all, dry_run),
        NetCommand::Status { network } => run_net_status(ctx, &network),
        NetCommand::Queue { link } => run_net_queue(ctx, &link),
        NetCommand::Packets { command } => run_net_packets(command, ctx),
        NetCommand::Nodelist { command } => run_nodelist(command, ctx),
        NetCommand::Areas { command } => match command {
            NetAreasCommand::List { network } => run_net_areas_list(ctx, network.as_deref()),
            NetAreasCommand::Subscribe {
                area_tag,
                link,
                network,
            } => run_net_area_subscription(ctx, &area_tag, &link, network.as_deref(), true),
            NetAreasCommand::Unsubscribe {
                area_tag,
                link,
                network,
            } => run_net_area_subscription(ctx, &area_tag, &link, network.as_deref(), false),
        },
        NetCommand::Links { command } => match command {
            NetLinksCommand::List { network } => run_net_links_list(ctx, network.as_deref()),
            NetLinksCommand::Show { link } => run_net_links_show(ctx, &link),
        },
        NetCommand::Logs { link, limit } => run_net_logs(ctx, link.as_deref(), limit),
    }
}

fn unsupported_network_operation(operation: &str, target: &str) -> CliResult<()> {
    Err(CliError::Message(format!(
        "net {operation} for {target:?} requires the v1.2 FTN tosser/scanner/BinkP session engine, which is not implemented yet"
    )))
}

fn run_net_poll(ctx: &AppContext, link: Option<String>, all: bool, dry_run: bool) -> CliResult<()> {
    if all && link.is_some() {
        return Err(CliError::Message(
            "net poll accepts either <link> or --all, not both".to_string(),
        ));
    }

    if dry_run {
        return run_net_poll_dry_run(ctx, link.as_deref(), all);
    }

    match (link.as_deref(), all) {
        (Some(link), false) => unsupported_network_operation("poll", link),
        (None, true) => unsupported_network_operation("poll", "all links"),
        (None, false) => Err(CliError::Message(
            "net poll requires <link> or --all".to_string(),
        )),
        (Some(_), true) => unreachable!("link/--all conflict checked above"),
    }
}

fn run_net_poll_dry_run(ctx: &AppContext, link: Option<&str>, all: bool) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    let links = if all {
        list_network_links(db.db())?
    } else {
        vec![require_network_link(
            &db,
            link.ok_or_else(|| {
                CliError::Message("net poll --dry-run requires <link> or --all".to_string())
            })?,
        )?]
    };

    let mut plans = Vec::with_capacity(links.len());
    for link in links {
        let profile = require_network_profile(&db, &link.network_id)?;
        let security_plan = transport_security_plan(&link.transport_security)
            .map_err(|error| CliError::Message(error.to_string()))?;
        let security_warning = security_plan.warning.clone();
        let outbound_ready = matching_packets(&db, &profile.id)?
            .into_iter()
            .filter(|packet| {
                packet.link_id.as_deref() == Some(link.id.as_str())
                    && packet.direction == "outbound"
                    && packet.status == "pending"
            })
            .count();
        plans.push(json!({
            "link": network_link_json(&link),
            "network": network_profile_json(&profile),
            "would_connect": link.enabled && profile.enabled,
            "outbound_ready": outbound_ready,
            "transport_security": link.transport_security,
            "transport_security_plan": {
                "requires_tls": security_plan.requires_tls,
                "attempts_tls": security_plan.attempts_tls,
                "allows_plaintext": security_plan.allows_plaintext,
                "warning": security_warning
            },
            "plaintext_warning": link.transport_security == "plaintext_legacy"
        }));
    }

    if ctx.json {
        print_json(&json!({"dry_run": true, "links": plans}))
    } else {
        for plan in plans {
            let link = &plan["link"];
            let network = &plan["network"];
            println!(
                "{}\tnetwork={}\twould_connect={}\toutbound_ready={}\tsecurity={}\tsecurity_warning={}",
                link["key"].as_str().unwrap_or("?"),
                network["key"].as_str().unwrap_or("?"),
                plan["would_connect"].as_bool().unwrap_or(false),
                plan["outbound_ready"].as_u64().unwrap_or(0),
                plan["transport_security"].as_str().unwrap_or("?"),
                plan["transport_security_plan"]["warning"]
                    .as_str()
                    .unwrap_or("")
            );
        }
        Ok(())
    }
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

fn run_net_queue(ctx: &AppContext, link: &str) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    let link_record = require_network_link(&db, link)?;
    let packets: Vec<_> = list_network_packets(db.db())?
        .into_iter()
        .filter(|packet| {
            packet.link_id.as_deref() == Some(link_record.id.as_str())
                && packet.direction == "outbound"
                && matches!(packet.status.as_str(), "pending" | "processing" | "failed")
        })
        .collect();

    if ctx.json {
        print_json(
            &json!({"link": network_link_json(&link_record), "queue": packets.iter().map(network_packet_json).collect::<Vec<_>>()}),
        )
    } else {
        for packet in packets {
            print_network_packet(&packet);
        }
        Ok(())
    }
}

fn run_net_packets(command: NetPacketsCommand, ctx: &AppContext) -> CliResult<()> {
    match command {
        NetPacketsCommand::Summary { network } => run_net_packets_summary(ctx, network.as_deref()),
        NetPacketsCommand::Show { packet_id } => run_net_packets_show(ctx, &packet_id),
        NetPacketsCommand::Retry { packet_id } => run_net_packets_retry(ctx, &packet_id),
        NetPacketsCommand::MarkQuarantined { packet_id, reason } => {
            run_net_packets_mark_quarantined(ctx, &packet_id, &reason)
        }
        NetPacketsCommand::Inbound { network, limit } => {
            run_net_packets_list(ctx, network.as_deref(), Some("inbound"), None, limit)
        }
        NetPacketsCommand::Outbound { network, limit } => {
            run_net_packets_list(ctx, network.as_deref(), Some("outbound"), None, limit)
        }
        NetPacketsCommand::Quarantine { network, limit } => {
            run_net_packets_list(ctx, network.as_deref(), None, Some("quarantined"), limit)
        }
    }
}

fn run_net_packets_summary(ctx: &AppContext, network: Option<&str>) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    let profile = match network {
        Some(network) => Some(require_network_profile(&db, network)?),
        None => None,
    };
    let summary =
        summarize_network_packets(db.db(), profile.as_ref().map(|profile| profile.id.as_str()))?;

    if ctx.json {
        print_json(&json!({
            "network": profile.as_ref().map(network_profile_json),
            "summary": summary.iter().map(packet_summary_json).collect::<Vec<_>>(),
            "counts": packet_summary_counts_json(&summary)
        }))
    } else {
        for row in summary {
            println!(
                "{}\t{}\tcount={}\tbytes={}",
                row.direction, row.status, row.count, row.total_size_bytes
            );
        }
        Ok(())
    }
}

fn run_net_packets_show(ctx: &AppContext, packet_id: &str) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    let packet = require_network_packet(&db, packet_id)?;

    if ctx.json {
        print_json(&json!({"packet": network_packet_json(&packet)}))
    } else {
        print_network_packet(&packet);
        Ok(())
    }
}

fn run_net_packets_retry(ctx: &AppContext, packet_id: &str) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    let packet = require_network_packet(&db, packet_id)?;
    if !packet_can_retry(&packet) {
        return Err(CliError::Message(format!(
            "packet {} has status {:?}; only failed or quarantined packets can be retried safely",
            packet.id, packet.status
        )));
    }
    let previous_status = packet.status.clone();
    if !requeue_network_packet(db.db(), &packet.id)? {
        return Err(CliError::Message(format!(
            "packet {:?} was not found during retry",
            packet.id
        )));
    }
    let updated = require_network_packet(&db, &packet.id)?;
    audit(
        &db,
        "network:packet:retry",
        None,
        None,
        &format!(
            "requeued packet {} ({}) from {} to pending; no files were moved",
            packet.id, packet.filename, previous_status
        ),
    )?;
    emit_ok(
        ctx.json,
        "network packet requeued",
        json!({
            "previous_status": previous_status,
            "packet": network_packet_json(&updated)
        }),
    )
}

fn run_net_packets_mark_quarantined(
    ctx: &AppContext,
    packet_id: &str,
    reason: &str,
) -> CliResult<()> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(CliError::Message(
            "net packets mark-quarantined requires a non-empty --reason".to_string(),
        ));
    }

    let db = open_database(&ctx.config)?;
    let packet = require_network_packet(&db, packet_id)?;
    let previous_status = packet.status.clone();
    if !mark_network_packet_quarantined(db.db(), &packet.id, reason)? {
        return Err(CliError::Message(format!(
            "packet {:?} was not found during quarantine",
            packet.id
        )));
    }
    let updated = require_network_packet(&db, &packet.id)?;
    audit(
        &db,
        "network:packet:mark-quarantined",
        None,
        None,
        &format!(
            "marked packet {} ({}) quarantined from {}: {}; no files were moved",
            packet.id, packet.filename, previous_status, reason
        ),
    )?;
    emit_ok(
        ctx.json,
        "network packet marked quarantined",
        json!({
            "previous_status": previous_status,
            "packet": network_packet_json(&updated)
        }),
    )
}

fn run_net_packets_list(
    ctx: &AppContext,
    network: Option<&str>,
    direction: Option<&str>,
    status: Option<&str>,
    limit: usize,
) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    let profile = match network {
        Some(network) => Some(require_network_profile(&db, network)?),
        None => None,
    };
    let packets: Vec<_> = list_network_packets(db.db())?
        .into_iter()
        .filter(|packet| {
            profile
                .as_ref()
                .is_none_or(|profile| packet.network_id == profile.id)
                && direction.is_none_or(|direction| packet.direction == direction)
                && status.is_none_or(|status| packet.status == status)
        })
        .take(limit)
        .collect();

    if ctx.json {
        print_json(
            &json!({"network": profile.as_ref().map(network_profile_json), "packets": packets.iter().map(network_packet_json).collect::<Vec<_>>()}),
        )
    } else {
        for packet in packets {
            print_network_packet(&packet);
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

fn run_net_area_subscription(
    ctx: &AppContext,
    area_tag: &str,
    link: &str,
    network: Option<&str>,
    subscribed: bool,
) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    let link_record = require_network_link(&db, link)?;
    let profile = match network {
        Some(network) => require_network_profile(&db, network)?,
        None => require_network_profile(&db, &link_record.network_id)?,
    };
    if link_record.network_id != profile.id {
        return Err(CliError::Message(format!(
            "link {} belongs to a different network profile",
            link_record.key
        )));
    }
    let area =
        find_network_area_by_tag_and_profile(db.db(), &profile.id, area_tag)?.ok_or_else(|| {
            CliError::Message(format!(
                "network area {area_tag:?} was not found for network {}",
                profile.key
            ))
        })?;
    let timestamp = current_timestamp(&db)?;

    if !set_network_subscription_status(
        db.db(),
        &area.id,
        &link_record.id,
        subscribed,
        &timestamp,
        "manual",
    )? {
        insert_network_subscription(
            db.db(),
            &NetworkSubscriptionRecord {
                id: generated_uuid(&db)?,
                area_id: area.id.clone(),
                link_id: link_record.id.clone(),
                subscribed,
                subscribed_at: timestamp.clone(),
                unsubscribed_at: (!subscribed).then_some(timestamp.clone()),
                source: "manual".to_string(),
            },
        )?;
    }

    let area_subscribed = subscribed
        || list_network_subscriptions(db.db())?
            .into_iter()
            .any(|subscription| subscription.area_id == area.id && subscription.subscribed);
    set_network_area_subscribed(db.db(), &area.id, area_subscribed)?;

    let action = if subscribed {
        "network:area:subscribe"
    } else {
        "network:area:unsubscribe"
    };
    audit(
        &db,
        action,
        None,
        None,
        &format!(
            "{} area {} for link {} on network {}",
            if subscribed {
                "subscribed"
            } else {
                "unsubscribed"
            },
            area.area_tag,
            link_record.key,
            profile.key
        ),
    )?;

    emit_ok(
        ctx.json,
        if subscribed {
            "network area subscribed"
        } else {
            "network area unsubscribed"
        },
        json!({
            "network": network_profile_json(&profile),
            "area": network_area_json(&area),
            "link": network_link_json(&link_record),
            "subscribed": subscribed
        }),
    )
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

fn run_net_links_show(ctx: &AppContext, link: &str) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    let link_record = require_network_link(&db, link)?;
    let profile = require_network_profile(&db, &link_record.network_id)?;
    let packets = list_network_packets(db.db())?
        .into_iter()
        .filter(|packet| packet.link_id.as_deref() == Some(link_record.id.as_str()))
        .collect::<Vec<_>>();
    let logs = list_network_poll_logs(db.db())?
        .into_iter()
        .filter(|log| log.link_id == link_record.id)
        .collect::<Vec<_>>();
    let subscriptions = list_network_subscriptions(db.db())?
        .into_iter()
        .filter(|subscription| subscription.link_id == link_record.id)
        .collect::<Vec<_>>();
    let last_poll = logs.first();
    let outbound_ready = packets
        .iter()
        .filter(|packet| packet.direction == "outbound" && packet.status == "pending")
        .count();
    let inbound_pending = packets
        .iter()
        .filter(|packet| packet.direction == "inbound" && packet.status == "pending")
        .count();
    let quarantined = packets
        .iter()
        .filter(|packet| packet.status == "quarantined")
        .count();

    if ctx.json {
        print_json(&json!({
            "link": network_link_json(&link_record),
            "network": network_profile_json(&profile),
            "last_poll": last_poll.map(poll_log_json),
            "counts": {
                "outbound_ready": outbound_ready,
                "inbound_pending": inbound_pending,
                "quarantined": quarantined,
                "subscriptions": subscriptions.len()
            },
            "plaintext_warning": link_record.transport_security == "plaintext_legacy"
        }))
    } else {
        println!(
            "{}\tnetwork={}\taddress={}\thost={}:{}\tenabled={}\tprofile_enabled={}",
            link_record.key,
            profile.key,
            link_record.address,
            link_record.host,
            link_record.binkp_port,
            link_record.enabled,
            profile.enabled
        );
        println!(
            "security={}\tcompression={}\tpoll_schedule_minutes={}\tplaintext_warning={}",
            link_record.transport_security,
            link_record.compression,
            link_record.poll_schedule_minutes,
            link_record.transport_security == "plaintext_legacy"
        );
        println!(
            "outbound_ready={outbound_ready}\tinbound_pending={inbound_pending}\tquarantined={quarantined}\tsubscriptions={}",
            subscriptions.len()
        );
        if let Some(log) = last_poll {
            println!(
                "last_poll={}\tstatus={}\terror={}",
                log.started_at,
                log.status,
                log.error_message.as_deref().unwrap_or("")
            );
        }
        Ok(())
    }
}

fn run_net_logs(ctx: &AppContext, link: Option<&str>, limit: usize) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    let link_record = match link {
        Some(link) => Some(require_network_link(&db, link)?),
        None => None,
    };
    let logs: Vec<_> = list_network_poll_logs(db.db())?
        .into_iter()
        .filter(|log| {
            link_record
                .as_ref()
                .is_none_or(|link| log.link_id == link.id)
        })
        .take(limit)
        .collect();

    if ctx.json {
        print_json(
            &json!({"link": link_record.as_ref().map(network_link_json), "poll_logs": logs.iter().map(poll_log_json).collect::<Vec<_>>()}),
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

fn require_network_packet(
    db: &oxidebbs_db::OxideDb,
    packet_id: &str,
) -> CliResult<NetworkPacketRecord> {
    find_network_packet_by_id(db.db(), packet_id)?
        .ok_or_else(|| CliError::Message(format!("network packet {packet_id:?} was not found")))
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

fn packet_can_retry(packet: &NetworkPacketRecord) -> bool {
    matches!(packet.status.as_str(), "failed" | "quarantined")
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

fn network_packet_json(packet: &NetworkPacketRecord) -> JsonValue {
    json!({
        "id": packet.id,
        "network_id": packet.network_id,
        "direction": packet.direction,
        "link_id": packet.link_id,
        "filename": packet.filename,
        "sha256": packet.sha256,
        "size_bytes": packet.size_bytes,
        "status": packet.status,
        "error_message": packet.error_message,
        "received_at": packet.received_at,
        "processed_at": packet.processed_at,
        "created_at": packet.created_at
    })
}

fn packet_summary_json(summary: &NetworkPacketSummaryRecord) -> JsonValue {
    json!({
        "direction": summary.direction,
        "status": summary.status,
        "count": summary.count,
        "total_size_bytes": summary.total_size_bytes
    })
}

fn packet_summary_counts_json(summary: &[NetworkPacketSummaryRecord]) -> JsonValue {
    let total_packets: i64 = summary.iter().map(|row| row.count).sum();
    let total_size_bytes: i64 = summary.iter().map(|row| row.total_size_bytes).sum();
    let failed: i64 = summary
        .iter()
        .filter(|row| row.status == "failed")
        .map(|row| row.count)
        .sum();
    let quarantined: i64 = summary
        .iter()
        .filter(|row| row.status == "quarantined")
        .map(|row| row.count)
        .sum();
    let pending: i64 = summary
        .iter()
        .filter(|row| row.status == "pending")
        .map(|row| row.count)
        .sum();

    json!({
        "total_packets": total_packets,
        "total_size_bytes": total_size_bytes,
        "pending": pending,
        "failed": failed,
        "quarantined": quarantined
    })
}

fn print_network_packet(packet: &NetworkPacketRecord) {
    println!(
        "{}\t{}\tstatus={}\tlink={}\tsize={}\terror={}",
        packet.created_at,
        packet.filename,
        packet.status,
        packet.link_id.as_deref().unwrap_or(""),
        packet.size_bytes,
        packet.error_message.as_deref().unwrap_or("")
    );
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

    #[test]
    fn network_packet_json_includes_queue_fields() {
        let packet = NetworkPacketRecord {
            id: "packet-id".to_string(),
            network_id: "net-id".to_string(),
            direction: "outbound".to_string(),
            link_id: Some("link-id".to_string()),
            filename: "outbound/00000001.pkt".to_string(),
            sha256: "hash".to_string(),
            size_bytes: 42,
            status: "pending".to_string(),
            error_message: None,
            received_at: None,
            processed_at: None,
            created_at: "now".to_string(),
        };

        let value = network_packet_json(&packet);

        assert_eq!(value["direction"], "outbound");
        assert_eq!(value["status"], "pending");
        assert_eq!(value["link_id"], "link-id");
    }

    #[test]
    fn packet_summary_counts_include_failure_and_quarantine_totals() {
        let summary = vec![
            NetworkPacketSummaryRecord {
                direction: "inbound".to_string(),
                status: "failed".to_string(),
                count: 2,
                total_size_bytes: 20,
            },
            NetworkPacketSummaryRecord {
                direction: "inbound".to_string(),
                status: "quarantined".to_string(),
                count: 1,
                total_size_bytes: 10,
            },
            NetworkPacketSummaryRecord {
                direction: "outbound".to_string(),
                status: "pending".to_string(),
                count: 3,
                total_size_bytes: 30,
            },
        ];

        let value = packet_summary_counts_json(&summary);

        assert_eq!(value["total_packets"], 6);
        assert_eq!(value["total_size_bytes"], 60);
        assert_eq!(value["failed"], 2);
        assert_eq!(value["quarantined"], 1);
        assert_eq!(value["pending"], 3);
    }

    #[test]
    fn packet_retry_is_limited_to_failed_or_quarantined_state() {
        let mut packet = NetworkPacketRecord {
            id: "packet-id".to_string(),
            network_id: "net-id".to_string(),
            direction: "inbound".to_string(),
            link_id: None,
            filename: "inbound/00000001.pkt".to_string(),
            sha256: "hash".to_string(),
            size_bytes: 42,
            status: "failed".to_string(),
            error_message: Some("bad".to_string()),
            received_at: None,
            processed_at: Some("now".to_string()),
            created_at: "now".to_string(),
        };

        assert!(packet_can_retry(&packet));
        packet.status = "quarantined".to_string();
        assert!(packet_can_retry(&packet));
        packet.status = "pending".to_string();
        assert!(!packet_can_retry(&packet));
        packet.status = "processed".to_string();
        assert!(!packet_can_retry(&packet));
    }
}
