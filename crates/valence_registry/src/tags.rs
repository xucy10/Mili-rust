use std::borrow::Cow;
use std::collections::BTreeMap;

use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use tracing::{error, info};
use valence_ident::Ident;
use valence_protocol::encode::{PacketWriter, WritePacket};
pub use valence_protocol::packets::play::synchronize_tags_s2c::RegistryMap;
use valence_protocol::packets::play::SynchronizeTagsS2c;
use valence_protocol::VarInt;
use valence_server_common::Server;

use crate::RegistrySet;

#[derive(Debug, Resource, Default)]
pub struct TagsRegistry {
    pub registries: RegistryMap,
    cached_packet: Vec<u8>,
}

pub(super) fn build(app: &mut App) {
    app.init_resource::<TagsRegistry>()
        .add_systems(PreStartup, init_tags_registry)
        .add_systems(PostUpdate, cache_tags_packet.in_set(RegistrySet));
}

impl TagsRegistry {
    fn build_synchronize_tags(&self) -> SynchronizeTagsS2c<'_> {
        SynchronizeTagsS2c {
            groups: Cow::Borrowed(&self.registries),
        }
    }

    /// Returns bytes of the cached [`SynchronizeTagsS2c`] packet.
    pub fn sync_tags_packet(&self) -> &[u8] {
        &self.cached_packet
    }
}

fn init_tags_registry(mut tags: ResMut<TagsRegistry>) {
    let tags_str = include_str!("../extracted/tags.json");
    let tags_json: serde_json::Value = match serde_json::from_str(tags_str) {
        Ok(v) => v,
        Err(e) => {
            error!("failed to parse tags.json: {e}");
            return;
        }
    };

    let registries_str = include_str!("../extracted/registries.json");
    let registries_json: serde_json::Value = match serde_json::from_str(registries_str) {
        Ok(v) => v,
        Err(e) => {
            error!("failed to parse registries.json: {e}");
            return;
        }
    };

    let mut registry_id_map: std::collections::HashMap<String, std::collections::HashMap<String, i32>> =
        std::collections::HashMap::new();

    if let Some(regs) = registries_json.as_object() {
        for (reg_name, reg_value) in regs {
            let mut id_map = std::collections::HashMap::new();
            if let Some(entries) = reg_value.get("entries").and_then(|e| e.as_object()) {
                for (entry_name, entry_value) in entries {
                    if let Some(protocol_id) = entry_value.get("protocol_id").and_then(|v| v.as_i64()) {
                        id_map.insert(entry_name.clone(), protocol_id as i32);
                    }
                }
            }
            registry_id_map.insert(reg_name.clone(), id_map);
        }
    }

    let tags_registries = match tags_json.as_object() {
        Some(obj) => obj,
        None => {
            error!("tags.json is not a JSON object");
            return;
        }
    };

    let mut result = RegistryMap::new();

    for (registry_name, tags_value) in tags_registries {
        let Some(tags_obj) = tags_value.as_object() else {
            continue;
        };

        let full_reg_name = if registry_name.contains(':') {
            registry_name.clone()
        } else {
            format!("minecraft:{registry_name}")
        };

        let reg_ident: Ident<String> = match Ident::new(full_reg_name.clone()) {
            Ok(id) => id.into(),
            Err(_) => continue,
        };

        let id_map = registry_id_map.get(&full_reg_name);

        let mut tag_map = BTreeMap::new();

        for (tag_name, entries_value) in tags_obj {
            let Some(entries_arr) = entries_value.as_array() else {
                continue;
            };

            let tag_ident: Ident<String> = match Ident::new(tag_name.clone()) {
                Ok(id) => id.into(),
                Err(_) => continue,
            };

            let entries: Vec<VarInt> = entries_arr
                .iter()
                .filter_map(|entry| {
                    let s = entry.as_str()?;
                    let entry_name = s.strip_prefix('#').unwrap_or(s);
                    if let Some(map) = id_map {
                        map.get(entry_name).map(|&id| VarInt(id))
                    } else {
                        None
                    }
                })
                .collect();

            if !entries.is_empty() {
                tag_map.insert(tag_ident, entries);
            }
        }

        if !tag_map.is_empty() {
            result.insert(reg_ident, tag_map);
        }
    }

    info!("loaded {} tag registries for play phase", result.len());
    tags.registries = result;
}

pub(crate) fn cache_tags_packet(server: Res<Server>, tags: ResMut<TagsRegistry>) {
    if tags.is_changed() {
        let tags = tags.into_inner();
        let packet = tags.build_synchronize_tags();
        let mut bytes = vec![];
        let mut writer = PacketWriter::new(&mut bytes, server.compression_threshold());

        writer.write_packet(&packet);
        tags.cached_packet = bytes;
    }
}

#[cfg(test)]
mod tests {
    /* TODO: move this to src/tests/
    #[test]
    fn smoke_test() {
        let mut app = bevy_app::App::new();
        app.add_plugins(RegistryPlugin);
        // app.insert_resource(Server::default());
        app.update();

        let tags_registry = app.world.get_resource::<TagsRegistry>().unwrap();
        let packet = tags_registry.build_synchronize_tags();
        assert!(!packet.registries.is_empty());
        assert!(!tags_registry.cached_packet.is_empty());
    }
    */
}