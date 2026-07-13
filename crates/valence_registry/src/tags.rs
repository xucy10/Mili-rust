use std::borrow::Cow;

use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use tracing::warn;
use valence_ident::Ident;
use valence_protocol::encode::{PacketWriter, WritePacket};
pub use valence_protocol::packets::play::synchronize_tags_s2c::RegistryMap;
use valence_protocol::packets::play::SynchronizeTagsS2c;
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
    let json_str = include_str!("../extracted/tags.json");
    let json: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(e) => {
            warn!("failed to parse tags.json: {e}");
            return;
        }
    };

    let registries = match json.as_object() {
        Some(obj) => obj,
        None => {
            warn!("tags.json is not a JSON object");
            return;
        }
    };

    let mut result = RegistryMap::new();

    for (registry_name, tags_value) in registries {
        let Some(tags_obj) = tags_value.as_object() else {
            continue;
        };

        let reg_ident: Ident<String> = match Ident::new(registry_name.clone()) {
            Ok(id) => id.into(),
            Err(_) => continue,
        };

        let mut tag_map = std::collections::BTreeMap::new();

        for (tag_name, entries_value) in tags_obj {
            let Some(entries_arr) = entries_value.as_array() else {
                continue;
            };

            let tag_ident: Ident<String> = match Ident::new(tag_name.clone()) {
                Ok(id) => id.into(),
                Err(_) => continue,
            };

            let mut resolved: Vec<Ident<String>> = Vec::new();

            for entry in entries_arr {
                let Some(s) = entry.as_str() else {
                    continue;
                };
                if let Some(ref_tag) = s.strip_prefix('#') {
                    let ref_ident: Ident<String> = match Ident::new(ref_tag.to_owned()) {
                        Ok(id) => id.into(),
                        Err(_) => continue,
                    };
                    if let Some(ref_entries) = tag_map.get(&ref_ident) {
                        resolved.extend(ref_entries.iter().cloned());
                    }
                } else {
                    match Ident::new(s.to_owned()) {
                        Ok(id) => resolved.push(id.into()),
                        Err(_) => continue,
                    }
                }
            }

            tag_map.insert(tag_ident, resolved);
        }

        if !tag_map.is_empty() {
            result.insert(reg_ident, tag_map);
        }
    }

    tracing::info!("loaded {} tag registries for play phase", result.len());
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