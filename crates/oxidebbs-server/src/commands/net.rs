use std::collections::BTreeMap;
use std::fs;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use clap::Subcommand;
use serde_json::{Value as JsonValue, json};

use oxidebbs_binkp::{
    BinkpClient, BinkpClientHandshake, BinkpOutboundFile, BinkpRetryPolicy, BinkpStream,
    BinkpTlsClientConfig, LinkSessionRegistry, connect_tls, transport_security_plan,
};
use oxidebbs_core::FtnAddress;
use oxidebbs_db::{
    NetworkAreaRecord, NetworkLinkRecord, NetworkNodelistRecord, NetworkPacketRecord,
    NetworkPacketSummaryRecord, NetworkPollLogRecord, NetworkProfileRecord,
    NetworkSubscriptionRecord, count_network_packets_before, delete_network_packets_older_than,
    find_network_area_by_tag_and_profile, find_network_link_by_key, find_network_nodelist_entry,
    find_network_packet_by_id, find_network_profile_by_id, find_network_profile_by_key,
    find_network_rescan_by_id, finish_network_packet, get_network_operations_stats,
    insert_network_poll_log, insert_network_subscription, list_network_areas, list_network_links,
    list_network_messages, list_network_nodelist_entries, list_network_packets,
    list_network_packets_for_retention, list_network_poll_logs, list_network_profiles,
    list_network_rescan_queue, list_network_subscriptions, mark_network_packet_quarantined,
    replace_network_nodelist_entries, requeue_network_packet, set_network_area_subscribed,
    set_network_subscription_status, summarize_network_packets, update_network_rescan_status,
};
use oxidebbs_ftn::{
    AreaFixCommand, FtnNodelistEntry, Scanner, ScannerPaths, Tosser, TosserPaths,
    apply_nodelist_diff_with_options, parse_areafix_commands, parse_nodelist,
};

use crate::sysop_cli::{
    AppContext, CliError, CliResult, audit, current_timestamp, emit_ok, generated_uuid,
    open_database, print_json,
};

static BINKP_LINK_SESSIONS: OnceLock<LinkSessionRegistry> = OnceLock::new();

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
    #[command(name = "areafix")]
    AreaFix {
        #[command(subcommand)]
        command: NetAreaFixCommand,
    },
    Rescan {
        #[command(subcommand)]
        command: NetRescanCommand,
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
        #[arg(long, default_value_t = false)]
        validate_crc: bool,
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
    Cleanup {
        #[arg(long)]
        network: Option<String>,
        #[arg(long)]
        archive_days: Option<u32>,
        #[arg(long)]
        delete_days: Option<u32>,
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
pub enum NetAreaFixCommand {
    Send {
        link: String,
        command: String,
        #[arg(long)]
        password: String,
        #[arg(long)]
        network: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum NetRescanCommand {
    List {
        #[arg(long)]
        network: Option<String>,
        #[arg(long)]
        status: Option<String>,
    },
    Process {
        rescan_id: String,
    },
    Cancel {
        rescan_id: String,
    },
}

pub fn run_net(command: NetCommand, ctx: &AppContext) -> CliResult<()> {
    match command {
        NetCommand::Toss { network } => run_net_toss(ctx, &network),
        NetCommand::Scan { network } => run_net_scan(ctx, &network),
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
        NetCommand::AreaFix { command } => run_net_areafix(command, ctx),
        NetCommand::Rescan { command } => match command {
            NetRescanCommand::List { network, status } => {
                run_net_rescan_list(ctx, network.as_deref(), status.as_deref())
            }
            NetRescanCommand::Process { rescan_id } => run_net_rescan_process(ctx, &rescan_id),
            NetRescanCommand::Cancel { rescan_id } => run_net_rescan_cancel(ctx, &rescan_id),
        },
    }
}

fn run_net_scan(ctx: &AppContext, network: &str) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    let profile = require_network_profile(&db, network)?;
    if !profile.enabled {
        return Err(CliError::Message(format!(
            "network profile {} is disabled; enable it before scanning outbound messages",
            profile.key
        )));
    }
    let paths = ScannerPaths::under_runtime(&ctx.config.paths.runtime, &profile.key);
    let scanner = Scanner::new(db.db(), profile.clone(), paths.clone());
    let result = scanner
        .scan()
        .map_err(|error| CliError::Message(error.to_string()))?;

    let netmail_materialized = scanner
        .materialize_outbound_netmail()
        .map_err(|error| CliError::Message(error.to_string()))?;

    audit(
        &db,
        "network:scan",
        None,
        None,
        &format!(
            "scanned outbound messages for network {} into {}; links={} packets={} messages={} skipped={} errors={} netmail_materialized={}",
            profile.key,
            paths.outbound_root.display(),
            result.links_scanned,
            result.packets_created,
            result.messages_scanned,
            result.messages_skipped,
            result.errors.len(),
            netmail_materialized
        ),
    )?;

    if ctx.json {
        print_json(&json!({
            "network": network_profile_json(&profile),
            "outbound_root": paths.outbound_root,
            "result": {
                "links_scanned": result.links_scanned,
                "packets_created": result.packets_created,
                "messages_scanned": result.messages_scanned,
                "messages_skipped": result.messages_skipped,
                "errors": result.errors,
                "netmail_materialized": netmail_materialized
            }
        }))
    } else {
        println!(
            "network={}\toutbound={}\tlinks={}\tpackets={}\tmessages={}\tskipped={}\terrors={}\tnetmail={}",
            profile.key,
            paths.outbound_root.display(),
            result.links_scanned,
            result.packets_created,
            result.messages_scanned,
            result.messages_skipped,
            result.errors.len(),
            netmail_materialized
        );
        for error in result.errors {
            println!("error={error}");
        }
        Ok(())
    }
}

fn run_net_toss(ctx: &AppContext, network: &str) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    let profile = require_network_profile(&db, network)?;
    if !profile.enabled {
        return Err(CliError::Message(format!(
            "network profile {} is disabled; enable it before tossing inbound packets",
            profile.key
        )));
    }
    let paths = TosserPaths::under_runtime(&ctx.config.paths.runtime, &profile.key);
    let result = Tosser::new(db.db(), profile.clone(), paths.clone())
        .toss()
        .map_err(|error| CliError::Message(error.to_string()))?;

    audit(
        &db,
        "network:toss",
        None,
        None,
        &format!(
            "tossed inbound packets for network {} from {}; files={} packets={} imported={} duplicates={} quarantined_packets={} quarantined_messages={} skipped={} errors={}",
            profile.key,
            paths.inbound_drop.display(),
            result.files_processed,
            result.packets_processed,
            result.messages_imported,
            result.messages_duplicate,
            result.packets_quarantined,
            result.messages_quarantined,
            result.messages_skipped,
            result.errors.len()
        ),
    )?;

    if ctx.json {
        print_json(&json!({
            "network": network_profile_json(&profile),
            "inbound_drop": paths.inbound_drop,
            "result": {
                "files_processed": result.files_processed,
                "packets_processed": result.packets_processed,
                "packets_quarantined": result.packets_quarantined,
                "messages_imported": result.messages_imported,
                "messages_duplicate": result.messages_duplicate,
                "messages_quarantined": result.messages_quarantined,
                "messages_skipped": result.messages_skipped,
                "errors": result.errors
            }
        }))
    } else {
        println!(
            "network={}\tinbound={}\tfiles={}\tpackets={}\timported={}\tduplicates={}\tpacket_quarantine={}\tmessage_quarantine={}\tskipped={}\terrors={}",
            profile.key,
            paths.inbound_drop.display(),
            result.files_processed,
            result.packets_processed,
            result.messages_imported,
            result.messages_duplicate,
            result.packets_quarantined,
            result.messages_quarantined,
            result.messages_skipped,
            result.errors.len()
        );
        for error in result.errors {
            println!("error={error}");
        }
        Ok(())
    }
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
        (Some(link), false) => run_net_poll_execute(ctx, Some(link), false),
        (None, true) => run_net_poll_execute(ctx, None, true),
        (None, false) => Err(CliError::Message(
            "net poll requires <link> or --all".to_string(),
        )),
        (Some(_), true) => unreachable!("link/--all conflict checked above"),
    }
}

fn run_net_poll_execute(ctx: &AppContext, link: Option<&str>, all: bool) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    let links = if all {
        list_network_links(db.db())?
            .into_iter()
            .filter(|link| link.enabled)
            .collect::<Vec<_>>()
    } else {
        vec![require_network_link(
            &db,
            link.ok_or_else(|| CliError::Message("net poll requires <link> or --all".to_string()))?,
        )?]
    };
    if links.is_empty() {
        return Err(CliError::Message(
            "no enabled network links are available to poll".to_string(),
        ));
    }

    let mut executions = Vec::with_capacity(links.len());
    for link in links {
        let profile = require_network_profile(&db, &link.network_id)?;
        let paths = TosserPaths::under_runtime(&ctx.config.paths.runtime, &profile.key);
        let execution = poll_link_once(&db, &profile, &link, &paths)?;
        audit(
            &db,
            "network:poll",
            None,
            None,
            &format!(
                "polled link {} on network {}; status={} sent_files={} received_files={} bytes_out={} bytes_in={}",
                link.key,
                profile.key,
                execution.status,
                execution.packets_out,
                execution.packets_in,
                execution.bytes_out,
                execution.bytes_in
            ),
        )?;
        executions.push((profile, link, execution));
    }

    if ctx.json {
        print_json(&json!({
            "polls": executions.iter().map(|(profile, link, execution)| {
                json!({
                    "network": network_profile_json(profile),
                    "link": network_link_json(link),
                    "status": execution.status,
                    "bytes_in": execution.bytes_in,
                    "bytes_out": execution.bytes_out,
                    "packets_in": execution.packets_in,
                    "packets_out": execution.packets_out,
                    "received_files": execution.received_files,
                    "error_message": execution.error_message
                })
            }).collect::<Vec<_>>()
        }))
    } else {
        for (profile, link, execution) in executions {
            println!(
                "{}\tnetwork={}\tstatus={}\tin_files={}\tout_files={}\tin_bytes={}\tout_bytes={}\terror={}",
                link.key,
                profile.key,
                execution.status,
                execution.packets_in,
                execution.packets_out,
                execution.bytes_in,
                execution.bytes_out,
                execution.error_message.as_deref().unwrap_or("")
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PollExecution {
    status: String,
    bytes_in: i64,
    bytes_out: i64,
    packets_in: i64,
    packets_out: i64,
    received_files: Vec<String>,
    error_message: Option<String>,
}

fn poll_link_once(
    db: &oxidebbs_db::OxideDb,
    profile: &NetworkProfileRecord,
    link: &NetworkLinkRecord,
    paths: &TosserPaths,
) -> CliResult<PollExecution> {
    let _session_permit = binkp_link_sessions()
        .try_acquire(&link.key)
        .map_err(|error| CliError::Message(error.to_string()))?;

    let retry_policy = BinkpRetryPolicy::default();
    let mut failed_attempts = 0;

    loop {
        let started_at = current_timestamp(db)?;
        let poll_result = execute_binkp_poll(db, profile, link, paths);

        match poll_result {
            Ok(mut execution) => {
                execution.status = "success".to_string();
                insert_poll_log(db, link, &started_at, &execution)?;
                return Ok(execution);
            }
            Err(error) => {
                failed_attempts += 1;

                if retry_policy.should_retry_after(failed_attempts) {
                    let delay = retry_policy
                        .delay_after_failure(failed_attempts)
                        .unwrap_or(Duration::from_secs(30));
                    eprintln!(
                        "Poll attempt {} failed for link {}: {}. Retrying in {:?}...",
                        failed_attempts, link.key, error, delay
                    );
                    std::thread::sleep(delay);
                    continue;
                }

                let execution = PollExecution {
                    status: "failed".to_string(),
                    bytes_in: 0,
                    bytes_out: 0,
                    packets_in: 0,
                    packets_out: 0,
                    received_files: Vec::new(),
                    error_message: Some(error.to_string()),
                };
                insert_poll_log(db, link, &started_at, &execution)?;
                return Err(error);
            }
        }
    }
}

fn binkp_link_sessions() -> &'static LinkSessionRegistry {
    BINKP_LINK_SESSIONS.get_or_init(LinkSessionRegistry::new)
}

fn execute_binkp_poll(
    db: &oxidebbs_db::OxideDb,
    profile: &NetworkProfileRecord,
    link: &NetworkLinkRecord,
    paths: &TosserPaths,
) -> CliResult<PollExecution> {
    let security_plan = transport_security_plan(&link.transport_security)
        .map_err(|error| CliError::Message(error.to_string()))?;

    let outbound_packets = list_network_packets(db.db())?
        .into_iter()
        .filter(|packet| {
            packet.network_id == profile.id
                && packet.link_id.as_deref() == Some(link.id.as_str())
                && packet.direction == "outbound"
                && packet.status == "pending"
        })
        .collect::<Vec<_>>();
    let outbound_files = outbound_files_from_packets(&outbound_packets)?;
    let bytes_out = outbound_files
        .iter()
        .map(|file| i64::try_from(file.bytes.len()).unwrap_or(i64::MAX))
        .sum::<i64>();

    let port = u16::try_from(link.binkp_port).map_err(|_| {
        CliError::Message(format!(
            "link {} has invalid BinkP port {}",
            link.key, link.binkp_port
        ))
    })?;
    let tcp_stream = TcpStream::connect((&*link.host, port))?;
    tcp_stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    tcp_stream.set_write_timeout(Some(Duration::from_secs(30)))?;

    let mut stream = if security_plan.attempts_tls {
        let tls_config = BinkpTlsClientConfig::default();
        match connect_tls(tcp_stream, &link.host, &tls_config) {
            Ok(tls_stream) => BinkpStream::tls(tls_stream),
            Err(tls_error) => {
                if security_plan.allows_plaintext {
                    eprintln!(
                        "TLS failed for link {} ({}); falling back to plaintext: {}",
                        link.key, link.host, tls_error
                    );
                    let fallback_stream = TcpStream::connect((&*link.host, port))?;
                    fallback_stream.set_read_timeout(Some(Duration::from_secs(30)))?;
                    fallback_stream.set_write_timeout(Some(Duration::from_secs(30)))?;
                    BinkpStream::plain(fallback_stream)
                } else {
                    return Err(CliError::Message(format!(
                        "TLS required for link {} but handshake failed: {}",
                        link.key, tls_error
                    )));
                }
            }
        }
    } else {
        BinkpStream::plain(tcp_stream)
    };

    let client = BinkpClient::new();
    let local_address = profile_address(profile);
    let handshake = BinkpClientHandshake::new(
        vec![local_address],
        (!link.password.is_empty()).then_some(link.password.clone()),
    );
    client
        .handshake(&mut stream, &handshake)
        .map_err(|error| CliError::Message(error.to_string()))?;
    client
        .send_batch_with_acknowledgements(&mut stream, &outbound_files)
        .map_err(|error| CliError::Message(error.to_string()))?;
    for packet in &outbound_packets {
        finish_network_packet(db.db(), &packet.id, "processed", None)?;
    }

    let inbound = client
        .receive_batch(&mut stream)
        .map_err(|error| CliError::Message(error.to_string()))?;
    fs::create_dir_all(&paths.inbound_drop)?;
    let mut received_files = Vec::with_capacity(inbound.len());
    let mut bytes_in = 0_i64;
    for file in inbound {
        bytes_in = bytes_in.saturating_add(i64::try_from(file.bytes.len()).unwrap_or(i64::MAX));
        let output_path = available_spool_destination(&paths.inbound_drop.join(&file.name));
        fs::write(&output_path, &file.bytes)?;
        received_files.push(output_path.display().to_string());
    }

    Ok(PollExecution {
        status: String::new(),
        bytes_in,
        bytes_out,
        packets_in: i64::try_from(received_files.len()).unwrap_or(i64::MAX),
        packets_out: i64::try_from(outbound_files.len()).unwrap_or(i64::MAX),
        received_files,
        error_message: None,
    })
}

fn outbound_files_from_packets(
    packets: &[NetworkPacketRecord],
) -> CliResult<Vec<BinkpOutboundFile>> {
    let mut files = Vec::with_capacity(packets.len());
    for packet in packets {
        let path = Path::new(&packet.filename);
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                CliError::Message(format!(
                    "outbound packet {} has no safe filename for BinkP",
                    packet.filename
                ))
            })?
            .to_string();
        let bytes = fs::read(path)?;
        let mtime = fs::metadata(path)
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs())
            .unwrap_or_else(current_unix_seconds);
        files.push(
            BinkpOutboundFile::new(name, mtime, bytes)
                .map_err(|error| CliError::Message(error.to_string()))?,
        );
    }
    Ok(files)
}

fn insert_poll_log(
    db: &oxidebbs_db::OxideDb,
    link: &NetworkLinkRecord,
    started_at: &str,
    execution: &PollExecution,
) -> CliResult<()> {
    insert_network_poll_log(
        db.db(),
        &NetworkPollLogRecord {
            id: generated_uuid(db)?,
            link_id: link.id.clone(),
            started_at: started_at.to_string(),
            ended_at: Some(current_timestamp(db)?),
            direction: "bidirectional".to_string(),
            status: execution.status.clone(),
            bytes_in: execution.bytes_in,
            bytes_out: execution.bytes_out,
            packets_in: execution.packets_in,
            packets_out: execution.packets_out,
            error_message: execution.error_message.clone(),
        },
    )?;
    Ok(())
}

fn current_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn available_spool_destination(destination: &Path) -> PathBuf {
    if !destination.exists() {
        return destination.to_path_buf();
    }
    let parent = destination.parent().unwrap_or_else(|| Path::new(""));
    let stem = destination
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("packet");
    let extension = destination
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| format!(".{extension}"))
        .unwrap_or_default();
    for index in 1.. {
        let candidate = parent.join(format!("{stem}.{index}{extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    destination.to_path_buf()
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
    let stats = get_network_operations_stats(db.db(), &profile.id)?;

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
            "packet_status": count_by_status(&packets),
            "operations_stats": {
                "packets_tossed": stats.packets_tossed,
                "packets_quarantined": stats.packets_quarantined,
                "packets_scanned": stats.packets_scanned,
                "messages_imported": stats.messages_imported,
                "messages_exported": stats.messages_exported,
                "duplicates_detected": stats.duplicates_detected,
                "polls_succeeded": stats.polls_succeeded,
                "polls_failed": stats.polls_failed,
                "bytes_received": stats.bytes_received,
                "bytes_sent": stats.bytes_sent
            }
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
        println!(
            "stats.tossed={}\tstats.quarantined={}\tstats.scanned={}\tstats.imported={}\tstats.exported={}",
            stats.packets_tossed,
            stats.packets_quarantined,
            stats.packets_scanned,
            stats.messages_imported,
            stats.messages_exported
        );
        println!(
            "stats.duplicates={}\tstats.polls_ok={}\tstats.polls_fail={}\tstats.bytes_in={}\tstats.bytes_out={}",
            stats.duplicates_detected,
            stats.polls_succeeded,
            stats.polls_failed,
            stats.bytes_received,
            stats.bytes_sent
        );
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
        NetPacketsCommand::Cleanup {
            network,
            archive_days,
            delete_days,
            dry_run,
        } => run_net_packets_cleanup(ctx, network.as_deref(), archive_days, delete_days, dry_run),
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

fn run_net_packets_cleanup(
    ctx: &AppContext,
    network: Option<&str>,
    archive_days: Option<u32>,
    delete_days: Option<u32>,
    dry_run: bool,
) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    let _profile = match network {
        Some(network) => Some(require_network_profile(&db, network)?),
        None => None,
    };

    // Get retention settings from config or use provided values
    let retention_config = ctx.config.network.retention.as_ref();
    let archive_days = archive_days
        .or_else(|| retention_config.map(|c| c.archive_days))
        .unwrap_or(30);
    let delete_days = delete_days
        .or_else(|| retention_config.map(|c| c.delete_days))
        .unwrap_or(90);

    // Calculate cutoff timestamps
    let now = current_timestamp(&db)?;
    let archive_cutoff = calculate_cutoff_timestamp(&now, archive_days)?;
    let delete_cutoff = calculate_cutoff_timestamp(&now, delete_days)?;

    if dry_run {
        // Count packets that would be affected
        let archive_count = count_network_packets_before(db.db(), &archive_cutoff)?;
        let delete_count = count_network_packets_before(db.db(), &delete_cutoff)?;

        // List some example packets
        let archive_examples = list_network_packets_for_retention(db.db(), &archive_cutoff, 10)?;
        let delete_examples = list_network_packets_for_retention(db.db(), &delete_cutoff, 10)?;

        if ctx.json {
            print_json(&json!({
                "dry_run": true,
                "archive_days": archive_days,
                "archive_cutoff": archive_cutoff,
                "archive_count": archive_count,
                "delete_days": delete_days,
                "delete_cutoff": delete_cutoff,
                "delete_count": delete_count,
                "archive_examples": archive_examples.iter().map(network_packet_json).collect::<Vec<_>>(),
                "delete_examples": delete_examples.iter().map(network_packet_json).collect::<Vec<_>>(),
            }))
        } else {
            println!("Packet retention cleanup (dry run)");
            println!(
                "Archive threshold: {} days (before {})",
                archive_days, archive_cutoff
            );
            println!("  Packets to archive: {}", archive_count);
            if !archive_examples.is_empty() {
                println!("  Examples:");
                for packet in archive_examples.iter().take(5) {
                    println!(
                        "    - {} ({}, {})",
                        packet.filename, packet.status, packet.created_at
                    );
                }
            }
            println!();
            println!(
                "Delete threshold: {} days (before {})",
                delete_days, delete_cutoff
            );
            println!("  Packets to delete: {}", delete_count);
            if !delete_examples.is_empty() {
                println!("  Examples:");
                for packet in delete_examples.iter().take(5) {
                    println!(
                        "    - {} ({}, {})",
                        packet.filename, packet.status, packet.created_at
                    );
                }
            }
            Ok(())
        }
    } else {
        // Actually delete packets
        let deleted_count = delete_network_packets_older_than(db.db(), &delete_cutoff)?;

        audit(
            &db,
            "network:packets:cleanup",
            None,
            None,
            &format!(
                "Deleted {} packets older than {} days",
                deleted_count, delete_days
            ),
        )?;

        if ctx.json {
            print_json(&json!({
                "dry_run": false,
                "delete_days": delete_days,
                "delete_cutoff": delete_cutoff,
                "deleted_count": deleted_count,
            }))
        } else {
            println!(
                "Deleted {} packets older than {} days (before {})",
                deleted_count, delete_days, delete_cutoff
            );
            Ok(())
        }
    }
}

fn calculate_cutoff_timestamp(_now: &str, days: u32) -> CliResult<String> {
    // Parse ISO 8601 timestamp and subtract days
    // Format: 2026-06-04T15:27:58-05:00 or similar
    let seconds_per_day = 86400;
    let cutoff_seconds = days as i64 * seconds_per_day;

    // Get current time in seconds since epoch
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| CliError::Message(format!("Failed to get current time: {}", e)))?
        .as_secs() as i64;

    let cutoff = now_secs - cutoff_seconds;

    // Format as ISO 8601
    // Simple format: YYYY-MM-DDTHH:MM:SSZ
    // For now, just return seconds since epoch with Z suffix
    // In production, use chrono or time crate for proper formatting
    Ok(format!("{}Z", cutoff))
}

fn run_nodelist(command: NodelistCommand, ctx: &AppContext) -> CliResult<()> {
    match command {
        NodelistCommand::Import { file, network } => run_nodelist_import(ctx, &file, network),
        NodelistCommand::ApplyDiff {
            file,
            base,
            network,
            validate_crc,
        } => run_nodelist_apply_diff(ctx, &file, &base, network, validate_crc),
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
    validate_crc: bool,
) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    let profile = resolve_network_profile(&db, network.as_deref())?;
    let base_bytes = fs::read(base)?;
    let diff_bytes = fs::read(file)?;
    let base_contents = String::from_utf8_lossy(&base_bytes);
    let diff_contents = String::from_utf8_lossy(&diff_bytes);
    let updated_contents =
        apply_nodelist_diff_with_options(&base_contents, &diff_contents, validate_crc)
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

fn run_net_areafix(command: NetAreaFixCommand, ctx: &AppContext) -> CliResult<()> {
    match command {
        NetAreaFixCommand::Send {
            link,
            command,
            password,
            network,
        } => run_net_areafix_send(ctx, &link, &command, &password, network.as_deref()),
    }
}

fn run_net_areafix_send(
    ctx: &AppContext,
    link: &str,
    command_body: &str,
    password: &str,
    network: Option<&str>,
) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    let link_record = require_network_link(&db, link)?;
    if link_record.password != password {
        audit(
            &db,
            "network:areafix:auth-failed",
            None,
            None,
            &format!("AreaFix authentication failed for link {}", link_record.key),
        )?;
        return Err(CliError::Message(
            "AreaFix password did not match the configured link password".to_string(),
        ));
    }

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

    let commands = parse_areafix_commands(command_body)
        .map_err(|error| CliError::Message(error.to_string()))?;
    let reply = execute_areafix_commands(&db, &profile, &link_record, &commands)?;

    if ctx.json {
        print_json(&json!({
            "network": network_profile_json(&profile),
            "link": network_link_json(&link_record),
            "reply": reply
        }))
    } else {
        println!("{reply}");
        Ok(())
    }
}

fn execute_areafix_commands(
    db: &oxidebbs_db::OxideDb,
    profile: &NetworkProfileRecord,
    link: &NetworkLinkRecord,
    commands: &[AreaFixCommand],
) -> CliResult<String> {
    let mut lines = vec![
        format!("AreaFix response for {}", link.address),
        format!("Network: {}", profile.key),
        String::new(),
    ];

    for command in commands {
        match command {
            AreaFixCommand::List => {
                lines.push("Available areas:".to_string());
                let areas = matching_areas(db, &profile.id)?;
                for area in areas {
                    lines.push(format!(
                        "{}\t{}\t{}",
                        area.area_tag,
                        if area.subscribed {
                            "active"
                        } else {
                            "available"
                        },
                        area.description
                    ));
                }
            }
            AreaFixCommand::Query => {
                lines.push("Subscribed areas:".to_string());
                let subscriptions = link_subscribed_areas(db, profile, link)?;
                if subscriptions.is_empty() {
                    lines.push("(none)".to_string());
                } else {
                    lines.extend(subscriptions.into_iter().map(|area| area.area_tag));
                }
            }
            AreaFixCommand::Help => {
                lines.push(
                    "Commands: %LIST, %QUERY, %HELP, +AREA.TAG, -AREA.TAG, +AREA.TAG !".to_string(),
                );
            }
            AreaFixCommand::Subscribe { area_tag, rescan } => {
                let area = require_network_area(db, profile, area_tag)?;
                set_link_subscription(db, &area, link, true, "areafix")?;
                audit(
                    db,
                    "network:areafix:subscribe",
                    None,
                    None,
                    &format!(
                        "AreaFix subscribed link {} to area {} on network {}",
                        link.key, area.area_tag, profile.key
                    ),
                )?;
                lines.push(format!("Subscribed {}", area.area_tag));
                if *rescan {
                    lines.push(format!(
                        "Rescan requested for {}; rescan queueing is not implemented yet",
                        area.area_tag
                    ));
                }
            }
            AreaFixCommand::Unsubscribe { area_tag } => {
                let area = require_network_area(db, profile, area_tag)?;
                set_link_subscription(db, &area, link, false, "areafix")?;
                audit(
                    db,
                    "network:areafix:unsubscribe",
                    None,
                    None,
                    &format!(
                        "AreaFix unsubscribed link {} from area {} on network {}",
                        link.key, area.area_tag, profile.key
                    ),
                )?;
                lines.push(format!("Unsubscribed {}", area.area_tag));
            }
        }
    }

    audit(
        db,
        "network:areafix:processed",
        None,
        None,
        &format!(
            "processed {} AreaFix command(s) for link {} on network {}",
            commands.len(),
            link.key,
            profile.key
        ),
    )?;

    Ok(lines.join("\n"))
}

fn run_net_rescan_list(
    ctx: &AppContext,
    network: Option<&str>,
    status: Option<&str>,
) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    let network_id = match network {
        Some(network) => {
            let profile = require_network_profile(&db, network)?;
            Some(profile.id)
        }
        None => None,
    };

    let rescans = list_network_rescan_queue(db.db(), network_id.as_deref(), status)?;

    if rescans.is_empty() {
        println!("No rescan requests found");
        return Ok(());
    }

    println!(
        "{:<36} {:<20} {:<20} {:<12} {:<24} {:<24}",
        "ID", "Network", "Link", "Area", "Requested", "Processed"
    );
    println!("{}", "-".repeat(136));

    for rescan in rescans {
        let network_name = find_network_profile_by_id(db.db(), &rescan.network_id)
            .ok()
            .flatten()
            .map(|p| p.key)
            .unwrap_or_else(|| rescan.network_id.clone());

        let link_name = find_network_link_by_id(db.db(), &rescan.link_id)
            .ok()
            .flatten()
            .map(|l| l.key)
            .unwrap_or_else(|| rescan.link_id.clone());

        println!(
            "{:<36} {:<20} {:<20} {:<12} {:<24} {:<24}",
            rescan.id,
            network_name,
            link_name,
            rescan.area_tag,
            rescan.requested_at,
            rescan.processed_at.as_deref().unwrap_or("-")
        );
    }

    Ok(())
}

fn run_net_rescan_process(ctx: &AppContext, rescan_id: &str) -> CliResult<()> {
    let db = open_database(&ctx.config)?;

    let rescan = find_network_rescan_by_id(db.db(), rescan_id)?
        .ok_or_else(|| CliError::Message(format!("rescan request {} not found", rescan_id)))?;

    if rescan.status != "pending" {
        return Err(CliError::Message(format!(
            "rescan request {} is not pending (status: {})",
            rescan_id, rescan.status
        )));
    }

    let profile = find_network_profile_by_id(db.db(), &rescan.network_id)?.ok_or_else(|| {
        CliError::Message(format!("network profile {} not found", rescan.network_id))
    })?;

    let link = find_network_link_by_id(db.db(), &rescan.link_id)?
        .ok_or_else(|| CliError::Message(format!("network link {} not found", rescan.link_id)))?;

    let area = find_network_area_by_tag_and_profile(db.db(), &profile.id, &rescan.area_tag)?
        .ok_or_else(|| {
            CliError::Message(format!(
                "network area {} not found for network {}",
                rescan.area_tag, profile.key
            ))
        })?;

    // Update status to processing
    let timestamp = current_timestamp(&db)?;
    update_network_rescan_status(db.db(), rescan_id, "processing", Some(&timestamp))?;

    // Perform the rescan by triggering a scan for this specific area and link
    let paths = ScannerPaths::under_runtime(&ctx.config.paths.runtime, &profile.key);
    let scanner = Scanner::new(db.db(), profile.clone(), paths);

    let result = scanner
        .rescan_for_link(&link, &area.area_tag)
        .map_err(|e| CliError::Message(e.to_string()))?;

    // Update status to completed
    let timestamp = current_timestamp(&db)?;
    update_network_rescan_status(db.db(), rescan_id, "completed", Some(&timestamp))?;

    audit(
        &db,
        "network:rescan:processed",
        None,
        None,
        &format!(
            "processed rescan request {} for area {} on link {} (network {}): {} packets created",
            rescan_id, area.area_tag, link.key, profile.key, result.packets_created
        ),
    )?;

    println!(
        "Rescan completed: {} packets created for area {} on link {}",
        result.packets_created, area.area_tag, link.key
    );

    Ok(())
}

fn run_net_rescan_cancel(ctx: &AppContext, rescan_id: &str) -> CliResult<()> {
    let db = open_database(&ctx.config)?;

    let rescan = find_network_rescan_by_id(db.db(), rescan_id)?
        .ok_or_else(|| CliError::Message(format!("rescan request {} not found", rescan_id)))?;

    if rescan.status != "pending" {
        return Err(CliError::Message(format!(
            "rescan request {} is not pending (status: {})",
            rescan_id, rescan.status
        )));
    }

    let timestamp = current_timestamp(&db)?;
    update_network_rescan_status(db.db(), rescan_id, "cancelled", Some(&timestamp))?;

    audit(
        &db,
        "network:rescan:cancelled",
        None,
        None,
        &format!(
            "cancelled rescan request {} for area {}",
            rescan_id, rescan.area_tag
        ),
    )?;

    println!("Rescan request {} cancelled", rescan_id);

    Ok(())
}

fn find_network_link_by_id(
    db: &oxidebbs_db::Db,
    link_id: &str,
) -> CliResult<Option<NetworkLinkRecord>> {
    // This is a helper function to find a link by ID
    // We'll use the existing list_network_links and filter
    let links = list_network_links(db)?;
    Ok(links.into_iter().find(|l| l.id == link_id))
}

fn require_network_area(
    db: &oxidebbs_db::OxideDb,
    profile: &NetworkProfileRecord,
    area_tag: &str,
) -> CliResult<NetworkAreaRecord> {
    find_network_area_by_tag_and_profile(db.db(), &profile.id, area_tag)?.ok_or_else(|| {
        CliError::Message(format!(
            "network area {area_tag:?} was not found for network {}",
            profile.key
        ))
    })
}

fn set_link_subscription(
    db: &oxidebbs_db::OxideDb,
    area: &NetworkAreaRecord,
    link: &NetworkLinkRecord,
    subscribed: bool,
    source: &str,
) -> CliResult<()> {
    let timestamp = current_timestamp(db)?;
    if !set_network_subscription_status(
        db.db(),
        &area.id,
        &link.id,
        subscribed,
        &timestamp,
        source,
    )? {
        insert_network_subscription(
            db.db(),
            &NetworkSubscriptionRecord {
                id: generated_uuid(db)?,
                area_id: area.id.clone(),
                link_id: link.id.clone(),
                subscribed,
                subscribed_at: timestamp.clone(),
                unsubscribed_at: (!subscribed).then_some(timestamp),
                source: source.to_string(),
            },
        )?;
    }

    let area_subscribed = subscribed
        || list_network_subscriptions(db.db())?
            .into_iter()
            .any(|subscription| subscription.area_id == area.id && subscription.subscribed);
    set_network_area_subscribed(db.db(), &area.id, area_subscribed)?;
    Ok(())
}

fn link_subscribed_areas(
    db: &oxidebbs_db::OxideDb,
    profile: &NetworkProfileRecord,
    link: &NetworkLinkRecord,
) -> CliResult<Vec<NetworkAreaRecord>> {
    let subscriptions = list_network_subscriptions(db.db())?;
    Ok(matching_areas(db, &profile.id)?
        .into_iter()
        .filter(|area| {
            subscriptions.iter().any(|subscription| {
                subscription.link_id == link.id
                    && subscription.area_id == area.id
                    && subscription.subscribed
            })
        })
        .collect())
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
    use oxidebbs_binkp::{BinkpOutboundFile, BinkpServer, BinkpServerHandshake};
    use oxidebbs_db::{
        MessageAreaRecord, OxideDb, insert_message_area, insert_network_area, insert_network_link,
        insert_network_packet, insert_network_profile, list_network_poll_logs,
        list_network_subscriptions,
    };
    use std::net::TcpListener;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

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

    #[test]
    fn poll_link_sends_ready_packet_and_receives_inbound_file() {
        let db = OxideDb::open_memory().expect("open db");
        let profile = test_profile();
        let mut link = test_link(&profile.id);
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake BinkP server");
        link.host = "127.0.0.1".to_string();
        link.binkp_port = i64::from(listener.local_addr().expect("addr").port());
        insert_network_profile(db.db(), &profile).expect("insert profile");
        insert_network_link(db.db(), &link).expect("insert link");
        let root = temp_root("poll");
        let outbound_dir = root.join("network/fidonet/outbound/hub/ready");
        fs::create_dir_all(&outbound_dir).expect("create outbound");
        let outbound_path = outbound_dir.join("00000001.pkt");
        fs::write(&outbound_path, b"outbound packet").expect("write outbound");
        insert_network_packet(
            db.db(),
            &NetworkPacketRecord {
                id: "00000000-0000-4000-8000-000000003003".to_string(),
                network_id: profile.id.clone(),
                direction: "outbound".to_string(),
                link_id: Some(link.id.clone()),
                filename: outbound_path.display().to_string(),
                sha256: "hash".to_string(),
                size_bytes: 15,
                status: "pending".to_string(),
                error_message: None,
                received_at: None,
                processed_at: None,
                created_at: "2026-06-04T00:00:00Z".to_string(),
            },
        )
        .expect("insert packet");

        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept fake BinkP client");
            let server = BinkpServer::new();
            server
                .accept_handshake(
                    &mut stream,
                    &BinkpServerHandshake::new(
                        vec!["1:105/42".to_string()],
                        Some("SECRET".to_string()),
                    ),
                )
                .expect("accept handshake");
            let files = server.receive_batch(&mut stream).expect("receive outbound");
            assert_eq!(files.len(), 1);
            assert_eq!(files[0].name, "00000001.pkt");
            assert_eq!(files[0].bytes, b"outbound packet");
            let inbound = BinkpOutboundFile::new("inbound.pkt", 1234, b"inbound packet".to_vec())
                .expect("inbound file");
            server
                .send_batch(&mut stream, &[inbound])
                .expect("send inbound");
        });

        let execution = poll_link_once(
            &db,
            &profile,
            &link,
            &TosserPaths::under_runtime(&root, "fidonet"),
        )
        .expect("poll link");
        server.join().expect("fake server joined");

        assert_eq!(execution.status, "success");
        assert_eq!(execution.packets_out, 1);
        assert_eq!(execution.packets_in, 1);
        assert!(
            root.join("network/fidonet/inbound/drop/inbound.pkt")
                .exists()
        );
        let packets = list_network_packets(db.db()).expect("list packets");
        assert_eq!(packets[0].status, "processed");
        let logs = list_network_poll_logs(db.db()).expect("list poll logs");
        assert_eq!(logs[0].status, "success");
        assert_eq!(logs[0].packets_in, 1);
        assert_eq!(logs[0].packets_out, 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn areafix_subscribe_mutates_subscription_and_returns_reply() {
        let db = OxideDb::open_memory().expect("open db");
        let profile = test_profile();
        let link = test_link(&profile.id);
        insert_message_area(
            db.db(),
            &MessageAreaRecord {
                id: "00000000-0000-4000-8000-000000003004".to_string(),
                key: "oxide.general".to_string(),
                name: "Oxide General".to_string(),
                description: "General".to_string(),
                kind: "echomail".to_string(),
                network_id: Some(profile.id.clone()),
                read_security_level: 0,
                post_security_level: 10,
                moderated: false,
                enabled: true,
            },
        )
        .expect("insert message area");
        insert_network_profile(db.db(), &profile).expect("insert profile");
        insert_network_link(db.db(), &link).expect("insert link");
        insert_network_area(
            db.db(),
            &NetworkAreaRecord {
                id: "00000000-0000-4000-8000-000000003005".to_string(),
                network_id: profile.id.clone(),
                area_tag: "OXIDE.GENERAL".to_string(),
                local_area_id: "00000000-0000-4000-8000-000000003004".to_string(),
                description: "General".to_string(),
                read_only: false,
                subscribed: false,
                created_at: "2026-06-04T00:00:00Z".to_string(),
                updated_at: "2026-06-04T00:00:00Z".to_string(),
            },
        )
        .expect("insert network area");

        let reply = execute_areafix_commands(
            &db,
            &profile,
            &link,
            &[AreaFixCommand::Subscribe {
                area_tag: "OXIDE.GENERAL".to_string(),
                rescan: true,
            }],
        )
        .expect("execute areafix");

        assert!(reply.contains("Subscribed OXIDE.GENERAL"));
        assert!(reply.contains("Rescan requested"));
        let subscriptions = list_network_subscriptions(db.db()).expect("subscriptions");
        assert_eq!(subscriptions.len(), 1);
        assert!(subscriptions[0].subscribed);
        assert_eq!(subscriptions[0].source, "areafix");
    }

    fn test_profile() -> NetworkProfileRecord {
        NetworkProfileRecord {
            id: "00000000-0000-4000-8000-000000003001".to_string(),
            key: "fidonet".to_string(),
            name: "FidoNet".to_string(),
            adapter: "legacy-ftn".to_string(),
            local_zone: 1,
            local_net: 105,
            local_node: 42,
            local_point: 0,
            enabled: true,
            created_at: "2026-06-04T00:00:00Z".to_string(),
            updated_at: "2026-06-04T00:00:00Z".to_string(),
        }
    }

    fn test_link(network_id: &str) -> NetworkLinkRecord {
        NetworkLinkRecord {
            id: "00000000-0000-4000-8000-000000003002".to_string(),
            key: "hub".to_string(),
            network_id: network_id.to_string(),
            address: "1:105/1".to_string(),
            host: "127.0.0.1".to_string(),
            binkp_port: 24554,
            password: "SECRET".to_string(),
            poll_schedule_minutes: 60,
            compression: "none".to_string(),
            transport_security: "plaintext_legacy".to_string(),
            enabled: true,
            created_at: "2026-06-04T00:00:00Z".to_string(),
            updated_at: "2026-06-04T00:00:00Z".to_string(),
        }
    }

    fn temp_root(test_name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("oxidebbs-net-{test_name}-{suffix}"))
    }
}
