use std::collections::{BTreeMap, BTreeSet};

use oxidebbs_db::{
    OxideDb, list_network_areas, list_network_duplicates, list_network_links,
    list_network_messages, list_network_nodelist_entries, list_network_packets,
    list_network_poll_logs, list_network_profiles,
};

use crate::SysopError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkProfileSummary {
    pub key: String,
    pub name: String,
    pub adapter: String,
    pub address: String,
    pub enabled: bool,
    pub links: usize,
    pub areas: usize,
    pub packets: usize,
    pub messages: usize,
    pub nodelist_entries: usize,
    pub last_poll_status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkDashboard {
    pub profiles: Vec<NetworkProfileSummary>,
    pub total_links: usize,
    pub total_areas: usize,
    pub total_packets: usize,
    pub total_messages: usize,
    pub total_nodelist_entries: usize,
    pub total_poll_logs: usize,
    pub failed_polls: usize,
    pub duplicate_events: usize,
    pub packet_status_counts: BTreeMap<String, usize>,
}

pub struct NetworkAdminService;

impl NetworkAdminService {
    pub fn load(db: &OxideDb) -> Result<NetworkDashboard, SysopError> {
        let inner = db.db();
        let profiles = list_network_profiles(inner)?;
        let links = list_network_links(inner)?;
        let areas = list_network_areas(inner)?;
        let packets = list_network_packets(inner)?;
        let messages = list_network_messages(inner)?;
        let nodelist = list_network_nodelist_entries(inner)?;
        let poll_logs = list_network_poll_logs(inner)?;
        let duplicates = list_network_duplicates(inner)?;

        let mut poll_status_by_link = BTreeMap::new();
        for poll in &poll_logs {
            poll_status_by_link
                .entry(poll.link_id.clone())
                .or_insert_with(|| poll.status.clone());
        }

        let mut packet_status_counts = BTreeMap::new();
        for packet in &packets {
            *packet_status_counts
                .entry(packet.status.clone())
                .or_insert(0) += 1;
        }

        let mut summaries = Vec::with_capacity(profiles.len());
        for profile in profiles {
            let profile_links = links
                .iter()
                .filter(|link| link.network_id == profile.id)
                .collect::<Vec<_>>();
            let link_ids = profile_links
                .iter()
                .map(|link| link.id.as_str())
                .collect::<BTreeSet<_>>();
            let last_poll_status = profile_links
                .iter()
                .find_map(|link| poll_status_by_link.get(&link.id).cloned());

            summaries.push(NetworkProfileSummary {
                key: profile.key,
                name: profile.name,
                adapter: profile.adapter,
                address: format!(
                    "{}:{}/{}.{}",
                    profile.local_zone, profile.local_net, profile.local_node, profile.local_point
                ),
                enabled: profile.enabled,
                links: profile_links.len(),
                areas: areas
                    .iter()
                    .filter(|area| area.network_id == profile.id)
                    .count(),
                packets: packets
                    .iter()
                    .filter(|packet| packet.network_id == profile.id)
                    .count(),
                messages: messages
                    .iter()
                    .filter(|message| message.network_id == profile.id)
                    .count(),
                nodelist_entries: nodelist
                    .iter()
                    .filter(|entry| entry.network_id == profile.id)
                    .count(),
                last_poll_status: last_poll_status.or_else(|| {
                    poll_logs
                        .iter()
                        .find(|poll| link_ids.contains(poll.link_id.as_str()))
                        .map(|poll| poll.status.clone())
                }),
            });
        }

        let failed_polls = poll_logs
            .iter()
            .filter(|poll| poll.status.eq_ignore_ascii_case("failed"))
            .count();

        Ok(NetworkDashboard {
            profiles: summaries,
            total_links: links.len(),
            total_areas: areas.len(),
            total_packets: packets.len(),
            total_messages: messages.len(),
            total_nodelist_entries: nodelist.len(),
            total_poll_logs: poll_logs.len(),
            failed_polls,
            duplicate_events: duplicates.len(),
            packet_status_counts,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::NetworkAdminService;

    #[test]
    fn load_empty_network_dashboard() {
        let db = oxidebbs_db::OxideDb::open_memory().expect("open test db");
        let dashboard = NetworkAdminService::load(&db).expect("load network dashboard");

        assert!(dashboard.profiles.is_empty());
        assert_eq!(dashboard.total_links, 0);
        assert_eq!(dashboard.total_areas, 0);
        assert_eq!(dashboard.total_packets, 0);
        assert_eq!(dashboard.total_poll_logs, 0);
        assert_eq!(dashboard.failed_polls, 0);
        assert!(dashboard.packet_status_counts.is_empty());
    }
}
