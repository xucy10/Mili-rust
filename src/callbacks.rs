use rand::Rng;
use valence::network::{async_trait, BroadcastToLan, ServerListPing, SharedNetworkState};
use valence::text::IntoText;
use valence::MINECRAFT_VERSION;

use crate::config::ServerConfig;

pub(crate) struct MiliCallbacks {
    pub(crate) motd: Vec<String>,
    pub(crate) max_players: u32,
}

impl From<&ServerConfig> for MiliCallbacks {
    fn from(config: &ServerConfig) -> Self {
        Self {
            motd: config.motd.clone(),
            max_players: config.max_players,
        }
    }
}

fn pick_motd(motd: &[String]) -> &str {
    if motd.is_empty() {
        "Mili-rust Server"
    } else if motd.len() == 1 {
        &motd[0]
    } else {
        let idx = rand::thread_rng().gen_range(0..motd.len());
        &motd[idx]
    }
}

#[async_trait]
impl valence::network::NetworkCallbacks for MiliCallbacks {
    async fn server_list_ping(
        &self,
        _shared: &SharedNetworkState,
        _remote_addr: std::net::SocketAddr,
        handshake_data: &valence::network::HandshakeData,
    ) -> ServerListPing {
        ServerListPing::Respond {
            online_players: 0,
            max_players: self.max_players as i32,
            player_sample: vec![],
            description: pick_motd(&self.motd).to_owned().into_text(),
            favicon_png: &[],
            version_name: MINECRAFT_VERSION.to_owned(),
            protocol: handshake_data.protocol_version,
        }
    }

    async fn broadcast_to_lan(&self, _shared: &SharedNetworkState) -> BroadcastToLan {
        BroadcastToLan::Enabled(pick_motd(&self.motd).into())
    }
}
