use oxidebbs_network::FtnAddress;

use crate::MessageAttribute;

/// A configured FTN link that can receive routed netmail.
///
/// Direct links match their exact FTN address. Hub links also advertise one or
/// more [`HubRouteScope`] values describing the destination zones or nets they
/// can carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FtnRouteLink {
    /// Stable operator-facing key for the configured link.
    pub key: String,
    /// FTN address of the linked node or hub.
    pub address: FtnAddress,
    /// Destination scopes this link can serve as a hub for.
    pub hub_scopes: Vec<HubRouteScope>,
}

impl FtnRouteLink {
    /// Build a direct-only route link.
    #[must_use]
    pub fn direct(key: impl Into<String>, address: FtnAddress) -> Self {
        Self {
            key: key.into(),
            address,
            hub_scopes: Vec::new(),
        }
    }

    /// Build a hub route link with one or more destination scopes.
    #[must_use]
    pub fn hub(
        key: impl Into<String>,
        address: FtnAddress,
        hub_scopes: Vec<HubRouteScope>,
    ) -> Self {
        Self {
            key: key.into(),
            address,
            hub_scopes,
        }
    }
}

/// Destination scope advertised by a hub route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HubRouteScope {
    /// The hub can accept mail for any destination.
    Any,
    /// The hub can accept mail for any net in the given zone.
    Zone(u16),
    /// The hub can accept mail for one exact zone/net pair.
    Net { zone: u16, net: u16 },
}

impl HubRouteScope {
    fn match_specificity(self, destination: &FtnAddress) -> Option<u8> {
        match self {
            Self::Net { zone, net } if zone == destination.zone && net == destination.net => {
                Some(3)
            }
            Self::Zone(zone) if zone == destination.zone => Some(2),
            Self::Any => Some(1),
            Self::Net { .. } | Self::Zone(_) => None,
        }
    }
}

/// Result of applying the netmail routing policy from ADR 0026.
///
/// Crash and hold are treated as explicit direct-link decisions: a destination
/// that matches a configured link returns [`RoutingDecision::Crash`] or
/// [`RoutingDecision::Hold`] when the corresponding attribute is set. Otherwise
/// the same destination returns [`RoutingDecision::Direct`]. Hub routing is used
/// only after local and direct-link checks fail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutingDecision {
    /// Destination is one of this system's local addresses.
    LocalDelivery { address: FtnAddress },
    /// Destination matches a configured direct link.
    Direct { link: FtnRouteLink },
    /// Destination matches a direct link and the message has the Crash bit set.
    Crash { link: FtnRouteLink },
    /// Destination matches a direct link and the message has the Hold bit set.
    Hold { link: FtnRouteLink },
    /// Destination should be sent to a hub while preserving the final address.
    RoutedViaHub {
        hub: FtnRouteLink,
        final_destination: FtnAddress,
    },
    /// No configured local, direct, or hub route was found.
    UnknownDestination { destination: FtnAddress },
}

/// Pure netmail router for local/direct/hub/crash/hold decisions.
///
/// The router does not mutate queues or compose packets. Runtime scanners and
/// tossers use the returned [`RoutingDecision`] to decide where to enqueue a
/// message and which FTN kludges or outbound flags to apply.
#[derive(Debug, Clone)]
pub struct NetmailRouter {
    local_addresses: Vec<FtnAddress>,
    links: Vec<FtnRouteLink>,
}

impl NetmailRouter {
    /// Create a router from local addresses and configured links.
    #[must_use]
    pub fn new(local_addresses: Vec<FtnAddress>, links: Vec<FtnRouteLink>) -> Self {
        Self {
            local_addresses,
            links,
        }
    }

    /// Route a netmail destination according to ADR 0026.
    ///
    /// The result is deterministic: exact local addresses win first, exact
    /// direct links win second, and hub links are selected by the most specific
    /// matching scope (`Net` before `Zone` before `Any`). Ties keep configured
    /// link order.
    #[must_use]
    pub fn route(&self, destination: &FtnAddress, attributes: MessageAttribute) -> RoutingDecision {
        if let Some(address) = self
            .local_addresses
            .iter()
            .find(|address| *address == destination)
        {
            return RoutingDecision::LocalDelivery {
                address: address.clone(),
            };
        }

        if let Some(link) = self.links.iter().find(|link| link.address == *destination) {
            let link = link.clone();
            if attributes.contains(MessageAttribute::CRASH) {
                return RoutingDecision::Crash { link };
            }
            if attributes.contains(MessageAttribute::HOLD) {
                return RoutingDecision::Hold { link };
            }
            return RoutingDecision::Direct { link };
        }

        if let Some(hub) = self.best_hub(destination) {
            return RoutingDecision::RoutedViaHub {
                hub,
                final_destination: destination.clone(),
            };
        }

        RoutingDecision::UnknownDestination {
            destination: destination.clone(),
        }
    }

    fn best_hub(&self, destination: &FtnAddress) -> Option<FtnRouteLink> {
        let mut best: Option<(u8, &FtnRouteLink)> = None;
        for link in &self.links {
            let Some(specificity) = link
                .hub_scopes
                .iter()
                .filter_map(|scope| scope.match_specificity(destination))
                .max()
            else {
                continue;
            };

            match best {
                Some((best_specificity, _link)) if best_specificity >= specificity => {}
                _ => best = Some((specificity, link)),
            }
        }

        best.map(|(_specificity, link)| link.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(raw: &str) -> FtnAddress {
        raw.parse()
            .unwrap_or_else(|error| panic!("invalid test address {raw}: {error}"))
    }

    fn router() -> NetmailRouter {
        NetmailRouter::new(
            vec![addr("21:1/100"), addr("21:1/100.1")],
            vec![
                FtnRouteLink::direct("uplink", addr("21:1/101")),
                FtnRouteLink::hub(
                    "net-hub",
                    addr("21:1/500"),
                    vec![HubRouteScope::Net { zone: 21, net: 200 }],
                ),
                FtnRouteLink::hub("zone-hub", addr("21:1/600"), vec![HubRouteScope::Zone(21)]),
                FtnRouteLink::hub("any-hub", addr("1:1/1"), vec![HubRouteScope::Any]),
            ],
        )
    }

    #[test]
    fn routes_local_delivery_before_links() {
        let router = NetmailRouter::new(
            vec![addr("21:1/100")],
            vec![FtnRouteLink::direct("self-link", addr("21:1/100"))],
        );

        assert_eq!(
            router.route(&addr("21:1/100"), MessageAttribute::NONE),
            RoutingDecision::LocalDelivery {
                address: addr("21:1/100")
            }
        );
    }

    #[test]
    fn routes_direct_link() {
        assert_eq!(
            router().route(&addr("21:1/101"), MessageAttribute::NONE),
            RoutingDecision::Direct {
                link: FtnRouteLink::direct("uplink", addr("21:1/101"))
            }
        );
    }

    #[test]
    fn routes_crash_direct_link() {
        assert_eq!(
            router().route(&addr("21:1/101"), MessageAttribute::CRASH),
            RoutingDecision::Crash {
                link: FtnRouteLink::direct("uplink", addr("21:1/101"))
            }
        );
    }

    #[test]
    fn routes_hold_direct_link() {
        assert_eq!(
            router().route(&addr("21:1/101"), MessageAttribute::HOLD),
            RoutingDecision::Hold {
                link: FtnRouteLink::direct("uplink", addr("21:1/101"))
            }
        );
    }

    #[test]
    fn routes_by_net_hub() {
        assert_eq!(
            router().route(&addr("21:200/300"), MessageAttribute::NONE),
            RoutingDecision::RoutedViaHub {
                hub: FtnRouteLink::hub(
                    "net-hub",
                    addr("21:1/500"),
                    vec![HubRouteScope::Net { zone: 21, net: 200 }],
                ),
                final_destination: addr("21:200/300")
            }
        );
    }

    #[test]
    fn routes_by_zone_hub() {
        assert_eq!(
            router().route(&addr("21:300/400"), MessageAttribute::NONE),
            RoutingDecision::RoutedViaHub {
                hub: FtnRouteLink::hub("zone-hub", addr("21:1/600"), vec![HubRouteScope::Zone(21)],),
                final_destination: addr("21:300/400")
            }
        );
    }

    #[test]
    fn routes_cross_zone_by_any_hub() {
        assert_eq!(
            router().route(&addr("46:10/20"), MessageAttribute::NONE),
            RoutingDecision::RoutedViaHub {
                hub: FtnRouteLink::hub("any-hub", addr("1:1/1"), vec![HubRouteScope::Any]),
                final_destination: addr("46:10/20")
            }
        );
    }

    #[test]
    fn reports_unknown_destination() {
        let router = NetmailRouter::new(vec![addr("21:1/100")], Vec::new());

        assert_eq!(
            router.route(&addr("99:1/1"), MessageAttribute::NONE),
            RoutingDecision::UnknownDestination {
                destination: addr("99:1/1")
            }
        );
    }

    #[test]
    fn direct_link_wins_over_hub_scope() {
        let router = NetmailRouter::new(
            vec![addr("21:1/100")],
            vec![
                FtnRouteLink::hub("zone-hub", addr("21:1/600"), vec![HubRouteScope::Zone(21)]),
                FtnRouteLink::direct("direct", addr("21:200/300")),
            ],
        );

        assert_eq!(
            router.route(&addr("21:200/300"), MessageAttribute::NONE),
            RoutingDecision::Direct {
                link: FtnRouteLink::direct("direct", addr("21:200/300"))
            }
        );
    }

    #[test]
    fn most_specific_hub_scope_wins() {
        let router = NetmailRouter::new(
            vec![addr("21:1/100")],
            vec![
                FtnRouteLink::hub("any-hub", addr("1:1/1"), vec![HubRouteScope::Any]),
                FtnRouteLink::hub("zone-hub", addr("21:1/600"), vec![HubRouteScope::Zone(21)]),
                FtnRouteLink::hub(
                    "net-hub",
                    addr("21:1/500"),
                    vec![HubRouteScope::Net { zone: 21, net: 200 }],
                ),
            ],
        );

        assert_eq!(
            router.route(&addr("21:200/300"), MessageAttribute::NONE),
            RoutingDecision::RoutedViaHub {
                hub: FtnRouteLink::hub(
                    "net-hub",
                    addr("21:1/500"),
                    vec![HubRouteScope::Net { zone: 21, net: 200 }],
                ),
                final_destination: addr("21:200/300")
            }
        );
    }

    #[test]
    fn equal_hub_scope_keeps_configured_order() {
        let router = NetmailRouter::new(
            vec![addr("21:1/100")],
            vec![
                FtnRouteLink::hub(
                    "first-net-hub",
                    addr("21:1/500"),
                    vec![HubRouteScope::Net { zone: 21, net: 200 }],
                ),
                FtnRouteLink::hub(
                    "second-net-hub",
                    addr("21:1/501"),
                    vec![HubRouteScope::Net { zone: 21, net: 200 }],
                ),
            ],
        );

        assert_eq!(
            router.route(&addr("21:200/300"), MessageAttribute::NONE),
            RoutingDecision::RoutedViaHub {
                hub: FtnRouteLink::hub(
                    "first-net-hub",
                    addr("21:1/500"),
                    vec![HubRouteScope::Net { zone: 21, net: 200 }],
                ),
                final_destination: addr("21:200/300")
            }
        );
    }
}
