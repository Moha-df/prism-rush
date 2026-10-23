//! Public list of open games, so friends can join with one click.
//!
//! We have no server of our own, so we use free public **MQTT brokers**
//! (a tiny publish/subscribe message service, no account needed):
//! - a host publishes a *retained* message on `prismrush/v<N>/games/<instance>`
//!   containing its name, player count and iroh address (how to reach it);
//! - every game subscribes to `prismrush/v<N>/games/+` and shows the list;
//! - the broker clears the message automatically if the host vanishes
//!   ("last will" message), and the host refreshes it every few seconds.
//!
//! Two brokers are used in parallel: if one is down, the other still works.
//! The actual game traffic never goes through MQTT: only this tiny ad.

use std::collections::HashMap;
use crossbeam_channel::{unbounded as channel, Receiver, Sender};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bevy::prelude::*;
use bevy_replicon_iroh::iroh::EndpointAddr;
use rumqttc::{Client, Event, LastWill, MqttOptions, Packet, QoS};
use serde::{Deserialize, Serialize};

use super::protocol::{MatchInfo, Phase, Player, MAX_PLAYERS, PROTOCOL_VERSION};
use super::NetRole;
use crate::config::AppConfig;

const BROKERS: [(&str, u16); 2] = [("broker.hivemq.com", 1883), ("broker.emqx.io", 1883)];
/// Ads older than this are ignored (host crashed without the broker noticing yet).
const STALE_SECS: u64 = 45;
const REPUBLISH_EVERY: f32 = 5.0;

fn topic_prefix() -> String {
    format!("prismrush/v{PROTOCOL_VERSION}/games")
}

/// What a host announces.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameAd {
    pub name: String,
    pub players: u8,
    pub max_players: u8,
    pub in_progress: bool,
    /// How to reach the host: iroh public key + relay URL + direct addresses.
    pub addr: EndpointAddr,
    /// Unix time of the announcement.
    pub time: u64,
}

impl GameAd {
    pub fn is_fresh(&self) -> bool {
        now_secs().saturating_sub(self.time) <= STALE_SECS
    }
}

pub fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Parses an incoming MQTT message into `(topic, ad)`. Empty payload = ad removed.
pub fn parse_ad(payload: &[u8]) -> Option<GameAd> {
    if payload.is_empty() {
        return None;
    }
    serde_json::from_slice(payload).ok()
}

/// The list of open games, plus what we announce ourselves.
#[derive(Resource)]
pub struct Discovery {
    /// Our own unique topic (one per running game instance).
    my_topic: String,
    clients: Vec<Client>,
    incoming: Receiver<(String, Option<GameAd>)>,
    /// Open games by topic.
    pub games: HashMap<String, GameAd>,
    /// The address peers can use to reach our server (once online).
    pub my_addr: Option<EndpointAddr>,
    publishing: bool,
    timer: f32,
    pub connected_brokers: usize,
    status_rx: Receiver<bool>,
}

impl Discovery {
    /// Fresh games other than our own, sorted by name.
    pub fn open_games(&self) -> Vec<(String, GameAd)> {
        let mut v: Vec<(String, GameAd)> = self
            .games
            .iter()
            .filter(|(topic, ad)| **topic != self.my_topic && ad.is_fresh())
            .map(|(t, a)| (t.clone(), a.clone()))
            .collect();
        v.sort_by(|a, b| a.1.name.cmp(&b.1.name).then(a.0.cmp(&b.0)));
        v
    }

    fn publish(&self, payload: Vec<u8>) {
        for c in &self.clients {
            let _ = c.try_publish(self.my_topic.clone(), QoS::AtLeastOnce, true, payload.clone());
        }
    }

    /// Removes our ad (we stopped hosting).
    pub fn withdraw(&mut self) {
        if self.publishing {
            self.publish(Vec::new());
            self.publishing = false;
        }
        self.my_addr = None;
    }
}

/// Starts one background thread per broker. Each thread keeps the connection
/// alive (auto-reconnect) and forwards received ads to the game.
fn start_discovery(mut commands: Commands) {
    let instance = format!("{:016x}", crate::shared::rng::hash64(now_secs() ^ u64::from(std::process::id()) << 20));
    let my_topic = format!("{}/{}", topic_prefix(), instance);
    let (tx, rx) = channel();
    let (status_tx, status_rx) = channel();
    let mut clients = Vec::new();
    for (i, (host, port)) in BROKERS.iter().enumerate() {
        let mut opts = MqttOptions::new(format!("prismrush-{instance}-{i}"), *host, *port);
        opts.set_keep_alive(Duration::from_secs(15));
        // If we disappear (crash, network loss), the broker erases our ad.
        opts.set_last_will(LastWill::new(my_topic.clone(), Vec::new(), QoS::AtLeastOnce, true));
        let (client, connection) = Client::new(opts, 32);
        let _ = client.try_subscribe(format!("{}/+", topic_prefix()), QoS::AtMostOnce);
        spawn_broker_thread(host, client.clone(), connection, tx.clone(), status_tx.clone());
        clients.push(client);
    }
    commands.insert_resource(Discovery {
        my_topic,
        clients,
        incoming: rx,
        games: HashMap::new(),
        my_addr: None,
        publishing: false,
        timer: 0.0,
        connected_brokers: 0,
        status_rx,
    });
}

fn spawn_broker_thread(
    host: &'static str,
    client: Client,
    mut connection: rumqttc::Connection,
    tx: Sender<(String, Option<GameAd>)>,
    status: Sender<bool>,
) {
    std::thread::Builder::new()
        .name(format!("mqtt-{host}"))
        .spawn(move || {
            for event in connection.iter() {
                match event {
                    Ok(Event::Incoming(Packet::ConnAck(_))) => {
                        info!("game list: connected to {host}");
                        // Subscriptions are lost on reconnect: subscribe again.
                        let _ = client.try_subscribe(format!("{}/+", topic_prefix()), QoS::AtMostOnce);
                        let _ = status.send(true);
                    }
                    Ok(Event::Incoming(Packet::Publish(p))) => {
                        if tx.send((p.topic.clone(), parse_ad(&p.payload))).is_err() {
                            break; // game closed
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        warn!("game list: {host} unreachable ({e}), retrying in 5 s");
                        let _ = status.send(false);
                        std::thread::sleep(Duration::from_secs(5));
                    }
                }
            }
        })
        .expect("spawn mqtt thread");
}

fn receive_ads(mut discovery: ResMut<Discovery>) {
    while let Ok((topic, ad)) = discovery.incoming.try_recv() {
        match ad {
            Some(ad) => {
                discovery.games.insert(topic, ad);
            }
            None => {
                discovery.games.remove(&topic);
            }
        }
    }
    while let Ok(up) = discovery.status_rx.try_recv() {
        if up {
            discovery.connected_brokers += 1;
        } else {
            discovery.connected_brokers = discovery.connected_brokers.saturating_sub(1);
        }
    }
}

/// Hosts (and dedicated servers) keep their ad up to date.
fn publish_ad(
    mut discovery: ResMut<Discovery>,
    role: Res<NetRole>,
    config: Res<AppConfig>,
    players: Query<&Player>,
    info: Query<&MatchInfo>,
    time: Res<Time<Real>>,
) {
    if !role.is_authority() {
        if discovery.publishing {
            discovery.withdraw();
        }
        return;
    }
    let Some(addr) = discovery.my_addr.clone() else { return };
    discovery.timer -= time.delta_secs();
    let player_count = players.iter().count() as u8;
    if discovery.timer > 0.0 {
        return;
    }
    discovery.timer = REPUBLISH_EVERY;
    let name = if *role == NetRole::Dedicated { "Serveur dédié".to_string() } else { format!("Partie de {}", config.name) };
    let ad = GameAd {
        name,
        players: player_count,
        max_players: MAX_PLAYERS as u8,
        in_progress: info.single().is_ok_and(|m| m.phase != Phase::Lobby),
        addr,
        time: now_secs(),
    };
    if let Ok(payload) = serde_json::to_vec(&ad) {
        discovery.publish(payload);
        if !discovery.publishing {
            info!("game announced in the public list as '{}'", ad.name);
        }
        discovery.publishing = true;
    }
}

/// Clears our ad when the game exits normally.
fn withdraw_on_exit(mut exits: MessageReader<AppExit>, mut discovery: ResMut<Discovery>) {
    if exits.read().next().is_some() {
        discovery.withdraw();
        // Give the MQTT threads a moment to send it.
        std::thread::sleep(Duration::from_millis(300));
    }
}

pub struct DiscoveryPlugin;

impl Plugin for DiscoveryPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, start_discovery)
            .add_systems(Update, (receive_ads, publish_ad).chain())
            .add_systems(Last, withdraw_on_exit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_replicon_iroh::iroh::SecretKey;

    #[test]
    fn ad_roundtrip_json() {
        let id = SecretKey::generate().public();
        let ad = GameAd {
            name: "Partie de Zoé".into(),
            players: 2,
            max_players: 4,
            in_progress: false,
            addr: EndpointAddr::new(id),
            time: now_secs(),
        };
        let bytes = serde_json::to_vec(&ad).unwrap();
        let back = parse_ad(&bytes).unwrap();
        assert_eq!(back.name, ad.name);
        assert_eq!(back.addr, ad.addr);
        assert!(back.is_fresh());
        assert!(parse_ad(b"").is_none());
        assert!(parse_ad(b"garbage").is_none());
    }

    #[test]
    fn stale_ads_are_ignored() {
        let ad = GameAd {
            name: "x".into(),
            players: 1,
            max_players: 4,
            in_progress: false,
            addr: EndpointAddr::new(SecretKey::generate().public()),
            time: now_secs() - 1000,
        };
        assert!(!ad.is_fresh());
    }
}
