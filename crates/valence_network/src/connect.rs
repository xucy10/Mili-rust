//! Handles new connections to the server and the log-in process.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::io;
use std::net::SocketAddr;
use std::sync::OnceLock;
use std::time::Duration;

use anyhow::{bail, ensure, Context};
use base64::prelude::*;
use hmac::digest::Update;
use hmac::{Hmac, Mac};
use num_bigint::BigInt;
use reqwest::StatusCode;
use rsa::Pkcs1v15Encrypt;
use serde::Deserialize;
use serde_json::{json, Value};
use sha1::Sha1;
use sha2::{Digest, Sha256};
use tokio::net::{TcpListener, TcpStream};
use tracing::{error, info, trace, warn};
use uuid::Uuid;
use valence_lang::keys;
use valence_protocol::nbt::{Compound, List, Value as NbtValue};
use valence_protocol::profile::Property;
use valence_protocol::Decode;
use valence_protocol::Encode;
use valence_protocol::Ident;
use valence_server::client::Properties;
use valence_server::protocol::packets::configuration::config_registry_data_s2c::RegistryEntry;
use valence_server::protocol::packets::configuration::config_update_tags_s2c::ConfigRegistryMap;
use valence_server::protocol::packets::configuration::{
    ConfigClientInformationC2s, ConfigCustomPayloadC2s, ConfigCustomPayloadS2c,
    ConfigFinishConfigurationC2s, ConfigFinishConfigurationS2c, ConfigRegistryDataS2c,
    ConfigSelectKnownPacksC2s, ConfigSelectKnownPacksS2c, ConfigUpdateEnabledFeaturesS2c,
    ConfigUpdateTagsS2c,
};
use valence_server::protocol::packets::handshaking::handshake_c2s::HandshakeNextState;
use valence_server::protocol::packets::handshaking::HandshakeC2s;
use valence_server::protocol::packets::login::{
    LoginAcknowledgedC2s, LoginCompressionS2c, LoginDisconnectS2c, LoginHelloC2s, LoginHelloS2c,
    LoginKeyC2s, LoginQueryRequestS2c, LoginQueryResponseC2s, LoginSuccessS2c,
};
use valence_server::protocol::packets::status::{
    QueryPingC2s, QueryPongS2c, QueryRequestC2s, QueryResponseS2c,
};
use valence_server::protocol::{PacketDecoder, PacketEncoder, RawBytes, VarInt};
use valence_server::text::{Color, IntoText};
use valence_server::{ident, Text, MINECRAFT_VERSION, PROTOCOL_VERSION};

use crate::legacy_ping::try_handle_legacy_ping;
use crate::packet_io::PacketIo;
use crate::{CleanupOnDrop, ConnectionMode, NewClientInfo, ServerListPing, SharedNetworkState};

struct CachedRegistryEntry {
    name: String,
    element: Option<Compound>,
}

struct CachedRegistryData {
    entries: Vec<(String, Vec<CachedRegistryEntry>)>,
}

static REGISTRY_DATA: OnceLock<CachedRegistryData> = OnceLock::new();

fn get_registry_data() -> &'static CachedRegistryData {
    REGISTRY_DATA.get_or_init(|| {
        let bytes = include_bytes!("../../valence_registry/extracted/registry_codec.dat");
        let compound = valence_protocol::nbt::from_binary(&mut bytes.as_slice())
            .expect("failed to decode vanilla registry codec")
            .0;

        let mut result = vec![];

        for (reg_name, reg_value) in compound {
            if reg_name == "minecraft:enchantment" {
                let enchantments = valence_registry::enchantment::load_enchantments();
                let entries: Vec<CachedRegistryEntry> = enchantments
                    .into_iter()
                    .map(|(name, element)| CachedRegistryEntry {
                        name,
                        element: Some(element),
                    })
                    .collect();
                result.push((reg_name, entries));
                continue;
            }

            let NbtValue::Compound(mut outer) = reg_value else {
                continue;
            };

            let Some(NbtValue::List(List::Compound(values))) = outer.remove("value") else {
                continue;
            };

            let entries: Vec<CachedRegistryEntry> = values
                .into_iter()
                .filter_map(|mut v| {
                    let NbtValue::String(name) = v.remove("name")? else {
                        return None;
                    };
                    let mut element = match v.remove("element")? {
                        NbtValue::Compound(c) => c,
                        _ => return None,
                    };

                    if reg_name == "minecraft:worldgen/biome" {
                        if let Some(NbtValue::Compound(effects)) = element.remove("effects") {
                            let mut cleaned = effects.clone();
                            cleaned.remove("fog_color");
                            cleaned.remove("sky_color");
                            cleaned.remove("water_fog_color");
                            element.insert("effects".to_string(), NbtValue::Compound(cleaned));
                        }
                    }

                    Some(CachedRegistryEntry { name, element: Some(element) })
                })
                .collect();

            result.push((reg_name, entries));
        }

        CachedRegistryData { entries: result }
    })
}

async fn send_registry_data(io: &mut PacketIo, core_known: bool) -> anyhow::Result<()> {
    let data = get_registry_data();

    if core_known {
        let all_reg_entries = get_all_registry_entries();
        let core_registries = get_core_registry_names();
        let jar_data = get_jar_dynamic_registries();

        // Track which registries we've sent to avoid duplicates
        let mut sent_registries = std::collections::HashSet::new();

        for (reg_name, entry_names) in all_reg_entries {
            let is_core = core_registries.contains(reg_name.as_str());

            let codec_entries: Option<&Vec<CachedRegistryEntry>> = data.entries.iter()
                .find(|(name, _)| name == reg_name)
                .map(|(_, entries)| entries);

            let jar_entries: Option<&Vec<(String, Compound)>> = jar_data.get(reg_name.as_str());

            let reg_entries: Vec<RegistryEntry<'static>> = if is_core {
                entry_names.iter().map(|name| RegistryEntry {
                    entry_id: Ident::new(name.as_str()).unwrap(),
                    data: None,
                }).collect()
            } else if let Some(entries) = jar_entries {
                entries.iter().map(|(name, element)| RegistryEntry {
                    entry_id: Ident::new(name.as_str()).unwrap(),
                    data: Some(element.clone()),
                }).collect()
            } else if let Some(entries) = codec_entries {
                entries.iter().map(|e| RegistryEntry {
                    entry_id: Ident::new(e.name.as_str()).unwrap(),
                    data: e.element.clone(),
                }).collect()
            } else {
                entry_names.iter().map(|name| RegistryEntry {
                    entry_id: Ident::new(name.as_str()).unwrap(),
                    data: Some(Compound::new()),
                }).collect()
            };

            sent_registries.insert(reg_name.clone());
            io.send_packet(&ConfigRegistryDataS2c {
                registry_id: Ident::new(reg_name.as_str()).unwrap(),
                entries: Cow::Owned(reg_entries),
            })
            .await?;
        }

        // Send dynamic registries from server.jar that are NOT in registries.json
        for (reg_name, entries) in jar_data {
            if !sent_registries.contains(reg_name) {
                let reg_entries: Vec<RegistryEntry<'static>> = entries.iter().map(|(name, element)| {
                    RegistryEntry {
                        entry_id: Ident::new(name.as_str()).unwrap(),
                        data: Some(element.clone()),
                    }
                }).collect();

                if !reg_entries.is_empty() {
                    sent_registries.insert(reg_name.clone());
                    io.send_packet(&ConfigRegistryDataS2c {
                        registry_id: Ident::new(reg_name.as_str()).unwrap(),
                        entries: Cow::Owned(reg_entries),
                    })
                    .await?;
                }
            }
        }

        // Send core registries from registry_codec that are not in registries.json
        // (e.g. dimension_type, worldgen/biome) - the client has built-in data for these
        for (reg_name, entries) in &data.entries {
            if !sent_registries.contains(reg_name.as_str()) && core_registries.contains(reg_name.as_str()) {
                let reg_entries: Vec<RegistryEntry<'static>> = entries
                    .iter()
                    .map(|e| RegistryEntry {
                        entry_id: Ident::new(e.name.as_str()).unwrap(),
                        data: None,
                    })
                    .collect();

                if !reg_entries.is_empty() {
                    sent_registries.insert(reg_name.clone());
                    io.send_packet(&ConfigRegistryDataS2c {
                        registry_id: Ident::new(reg_name.as_str()).unwrap(),
                        entries: Cow::Owned(reg_entries),
                    })
                    .await?;
                }
            }
        }
    } else {
        for (reg_name, entries) in &data.entries {
            if reg_name == "minecraft:enchantment" {
                continue;
            }

            let reg_entries: Vec<RegistryEntry<'static>> = entries
                .iter()
                .map(|e| RegistryEntry {
                    entry_id: Ident::new(e.name.as_str()).unwrap(),
                    data: e.element.clone(),
                })
                .collect();

            io.send_packet(&ConfigRegistryDataS2c {
                registry_id: Ident::new(reg_name.as_str()).unwrap(),
                entries: Cow::Owned(reg_entries),
            })
            .await?;
        }
    }
    Ok(())
}

fn get_jar_dynamic_registries() -> &'static std::collections::HashMap<String, Vec<(String, Compound)>> {
    static DATA: OnceLock<std::collections::HashMap<String, Vec<(String, Compound)>>> = OnceLock::new();
    DATA.get_or_init(|| {
        let jar_bytes: &[u8] = include_bytes!("../../../server.jar");
        let reader = std::io::Cursor::new(jar_bytes);
        let mut archive = match zip::ZipArchive::new(reader) {
            Ok(a) => a,
            Err(e) => {
                error!("failed to open server.jar: {e}");
                return std::collections::HashMap::new();
            }
        };

        let dynamic_regs = [
            "cat_variant", "cat_sound_variant",
            "chicken_variant", "chicken_sound_variant",
            "cow_variant", "cow_sound_variant",
            "frog_variant",
            "painting_variant",
            "pig_variant", "pig_sound_variant",
            "wolf_variant", "wolf_sound_variant",
            "zombie_nautilus_variant",
            "banner_pattern",
            "damage_type",
            "instrument",
            "dialog",
            "timeline",
            "world_clock",
            "trim_material",
            "trim_pattern",
            "jukebox_song",
            // enchantment is core, handled by client built-in data
        ];

        let mut result = std::collections::HashMap::new();

        // The server.jar is a bundler. The actual data is in a nested version jar.
        // Read versions.list to find the path, e.g. "26.2/server-26.2.jar".
        let mut target_bytes: Vec<u8> = jar_bytes.to_vec();
        let mut version_jar_path = String::new();
        for i in 0..archive.len() {
            let mut file = match archive.by_index(i) {
                Ok(f) => f,
                Err(_) => continue,
            };
            let name = file.name().to_string();
            if name == "META-INF/versions.list" {
                let mut content = String::new();
                if std::io::Read::read_to_string(&mut file, &mut content).is_ok() {
                    // Format: "<sha256>\t<version>\t<path>"
                    if let Some(path) = content.split('\t').nth(2) {
                        version_jar_path = format!("META-INF/versions/{}", path.trim());
                    }
                }
            }
        }

        if !version_jar_path.is_empty() {
            for i in 0..archive.len() {
                let mut file = match archive.by_index(i) {
                    Ok(f) => f,
                    Err(_) => continue,
                };
                if file.name() == version_jar_path {
                    let mut buf = vec![];
                    if std::io::Read::read_to_end(&mut file, &mut buf).is_ok() {
                        info!("extracted version jar: {}", version_jar_path);
                        target_bytes = buf;
                    }
                    break;
                }
            }
        }

        let target_reader = std::io::Cursor::new(&target_bytes);
        let mut target_archive = match zip::ZipArchive::new(target_reader) {
            Ok(a) => a,
            Err(e) => {
                error!("failed to open target archive: {e}");
                return std::collections::HashMap::new();
            }
        };

        for reg in &dynamic_regs {
            let prefix = format!("data/minecraft/{reg}/");
            let mut entries = vec![];

            for i in 0..target_archive.len() {
                let mut file = match target_archive.by_index(i) {
                    Ok(f) => f,
                    Err(_) => continue,
                };
                let name = file.name().to_string();
                if name.starts_with(&prefix) && name.ends_with(".json") {
                    let entry_id = name.split('/').last().unwrap().replace(".json", "");
                    let mut content = String::new();
                    if std::io::Read::read_to_string(&mut file, &mut content).is_err() {
                        continue;
                    }
                    if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&content) {
                        let compound = json_to_nbt_compound(&json_val);
                        entries.push((format!("minecraft:{entry_id}"), compound));
                    }
                }
            }

            if !entries.is_empty() {
                entries.sort_by(|a, b| a.0.cmp(&b.0));
                result.insert(format!("minecraft:{reg}"), entries);
            }
        }

        info!("loaded {} dynamic registries from server.jar", result.len());
        result
    })
}

fn get_jar_dynamic_tags() -> &'static std::collections::HashMap<String, std::collections::HashMap<String, Vec<String>>> {
    static DATA: OnceLock<std::collections::HashMap<String, std::collections::HashMap<String, Vec<String>>>> = OnceLock::new();
    DATA.get_or_init(|| {
        let jar_bytes: &[u8] = include_bytes!("../../../server.jar");
        let reader = std::io::Cursor::new(jar_bytes);
        let mut archive = match zip::ZipArchive::new(reader) {
            Ok(a) => a,
            Err(e) => {
                error!("failed to open server.jar for tags: {e}");
                return std::collections::HashMap::new();
            }
        };

        let tag_registries = [
            "banner_pattern",
            "cat_variant",
            "painting_variant",
            "damage_type",
            "instrument",
            "trim_material",
            "trim_pattern",
            "world_clock",
            "jukebox_song",
            "dialog",
            "timeline",
        ];

        let mut target_bytes: Vec<u8> = jar_bytes.to_vec();
        let mut version_jar_path = String::new();
        for i in 0..archive.len() {
            let mut file = match archive.by_index(i) {
                Ok(f) => f,
                Err(_) => continue,
            };
            let name = file.name().to_string();
            if name == "META-INF/versions.list" {
                let mut content = String::new();
                if std::io::Read::read_to_string(&mut file, &mut content).is_ok() {
                    if let Some(path) = content.split('\t').nth(2) {
                        version_jar_path = format!("META-INF/versions/{}", path.trim());
                    }
                }
            }
        }

        if !version_jar_path.is_empty() {
            for i in 0..archive.len() {
                let mut file = match archive.by_index(i) {
                    Ok(f) => f,
                    Err(_) => continue,
                };
                if file.name() == version_jar_path {
                    let mut buf = vec![];
                    if std::io::Read::read_to_end(&mut file, &mut buf).is_ok() {
                        target_bytes = buf;
                    }
                    break;
                }
            }
        }

        let target_reader = std::io::Cursor::new(&target_bytes);
        let mut target_archive = match zip::ZipArchive::new(target_reader) {
            Ok(a) => a,
            Err(e) => {
                error!("failed to open target archive for tags: {e}");
                return std::collections::HashMap::new();
            }
        };

        let mut result: std::collections::HashMap<String, std::collections::HashMap<String, Vec<String>>> =
            std::collections::HashMap::new();

        for reg in &tag_registries {
            let prefix = format!("data/minecraft/tags/{reg}/");
            let mut tags: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();

            for i in 0..target_archive.len() {
                let mut file = match target_archive.by_index(i) {
                    Ok(f) => f,
                    Err(_) => continue,
                };
                let name = file.name().to_string();
                if name.starts_with(&prefix) && name.ends_with(".json") {
                    // Extract tag name from path: data/minecraft/tags/banner_pattern/pattern_item/flower.json
                    // -> pattern_item/flower
                    let relative = name[prefix.len()..].replace(".json", "");
                    let tag_name = format!("minecraft:{relative}");

                    let mut content = String::new();
                    if std::io::Read::read_to_string(&mut file, &mut content).is_err() {
                        continue;
                    }
                    if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&content) {
                        if let Some(values) = json_val.get("values").and_then(|v| v.as_array()) {
                            let entries: Vec<String> = values
                                .iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect();
                            if !entries.is_empty() {
                                tags.insert(tag_name, entries);
                            }
                        }
                    }
                }
            }

            if !tags.is_empty() {
                result.insert(format!("minecraft:{reg}"), tags);
            }
        }

        info!("loaded {} tag groups from server.jar", result.len());
        result
    })
}

fn json_to_nbt_compound(value: &serde_json::Value) -> Compound {
    let mut compound = Compound::new();
    if let serde_json::Value::Object(map) = value {
        for (k, v) in map {
            compound.insert(k.clone(), json_to_nbt_value(v));
        }
    }
    compound
}

fn json_to_nbt_value(value: &serde_json::Value) -> NbtValue {
    match value {
        serde_json::Value::Null => NbtValue::Compound(Compound::new()),
        serde_json::Value::Bool(b) => NbtValue::Byte(if *b { 1 } else { 0 }),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                if i >= i32::MIN as i64 && i <= i32::MAX as i64 {
                    NbtValue::Int(i as i32)
                } else {
                    NbtValue::Long(i)
                }
            } else if let Some(f) = n.as_f64() {
                NbtValue::Double(f)
            } else {
                NbtValue::Int(0)
            }
        }
        serde_json::Value::String(s) => NbtValue::String(s.clone()),
        serde_json::Value::Array(arr) => {
            if arr.is_empty() {
                NbtValue::List(List::End)
            } else {
                match &arr[0] {
                    serde_json::Value::String(_) => {
                        let list: Vec<String> = arr
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect();
                        NbtValue::List(List::String(list))
                    }
                    serde_json::Value::Number(_) => {
                        if arr[0].as_i64().is_some() {
                            let list: Vec<i32> = arr
                                .iter()
                                .filter_map(|v| v.as_i64().map(|i| i as i32))
                                .collect();
                            NbtValue::List(List::Int(list))
                        } else {
                            let list: Vec<f64> = arr
                                .iter()
                                .filter_map(|v| v.as_f64())
                                .collect();
                            NbtValue::List(List::Double(list))
                        }
                    }
                    serde_json::Value::Bool(_) => {
                        let list: Vec<i8> = arr
                            .iter()
                            .filter_map(|v| v.as_bool().map(|b| if b { 1 } else { 0 }))
                            .collect();
                        NbtValue::List(List::Byte(list))
                    }
                    _ => {
                        let list: Vec<Compound> =
                            arr.iter().map(|v| json_to_nbt_compound(v)).collect();
                        NbtValue::List(List::Compound(list))
                    }
                }
            }
        }
        serde_json::Value::Object(_) => NbtValue::Compound(json_to_nbt_compound(value)),
    }
}

fn get_core_registry_names() -> &'static std::collections::HashSet<&'static str> {
    static NAMES: OnceLock<std::collections::HashSet<&'static str>> = OnceLock::new();
    NAMES.get_or_init(|| {
        [
            "minecraft:activity",
            "minecraft:attribute",
            "minecraft:attribute_type",
            "minecraft:block",
            "minecraft:block_entity_type",
            "minecraft:block_predicate_type",
            "minecraft:block_type",
            "minecraft:chunk_status",
            "minecraft:command_argument_type",
            "minecraft:consume_effect_type",
            "minecraft:creative_mode_tab",
            "minecraft:custom_stat",
            "minecraft:data_component_predicate_type",
            "minecraft:data_component_type",
            "minecraft:enchantment_effect_component_type",
            "minecraft:enchantment_entity_effect_type",
            "minecraft:enchantment_level_based_value_type",
            "minecraft:enchantment_location_based_effect_type",
            "minecraft:enchantment_provider_type",
            "minecraft:enchantment_value_effect_type",
            "minecraft:entity_sub_predicate_type",
            "minecraft:entity_type",
            "minecraft:float_provider_type",
            "minecraft:fluid",
            "minecraft:game_event",
            "minecraft:game_rule",
            "minecraft:height_provider_type",
            "minecraft:int_provider_type",
            "minecraft:item",
            "minecraft:loot_condition_type",
            "minecraft:loot_function_type",
            "minecraft:loot_nbt_provider_type",
            "minecraft:loot_number_provider_type",
            "minecraft:loot_pool_entry_type",
            "minecraft:loot_score_provider_type",
            "minecraft:map_decoration_type",
            "minecraft:memory_module_type",
            "minecraft:menu",
            "minecraft:mob_effect",
            "minecraft:number_format_type",
            "minecraft:particle_type",
            "minecraft:point_of_interest_type",
            "minecraft:position_source_type",
            "minecraft:potion",
            "minecraft:recipe_book_category",
            "minecraft:recipe_display",
            "minecraft:recipe_serializer",
            "minecraft:recipe_type",
            "minecraft:rule_block_entity_modifier",
            "minecraft:rule_test",
            "minecraft:sensor_type",
            "minecraft:slot_display",
            "minecraft:slot_source_type",
            "minecraft:sound_event",
            "minecraft:stat_type",
            "minecraft:trigger_type",
            "minecraft:villager_profession",
            "minecraft:villager_type",
            "minecraft:worldgen/biome_source",
            "minecraft:worldgen/block_state_provider_type",
            "minecraft:worldgen/carver",
            "minecraft:worldgen/chunk_generator",
            "minecraft:worldgen/density_function_type",
            "minecraft:worldgen/feature",
            "minecraft:worldgen/feature_size_type",
            "minecraft:worldgen/foliage_placer_type",
            "minecraft:worldgen/material_condition",
            "minecraft:worldgen/material_rule",
            "minecraft:worldgen/placement_modifier_type",
            "minecraft:worldgen/pool_alias_binding",
            "minecraft:worldgen/root_placer_type",
            "minecraft:worldgen/structure_piece",
            "minecraft:worldgen/structure_placement",
            "minecraft:worldgen/structure_pool_element",
            "minecraft:worldgen/structure_processor",
            "minecraft:worldgen/structure_type",
            "minecraft:worldgen/tree_decorator_type",
            "minecraft:worldgen/trunk_placer_type",
            "minecraft:dimension_type",
            "minecraft:worldgen/biome",
        ].into_iter().collect()
    })
}

fn get_all_registry_entries() -> &'static Vec<(String, Vec<String>)> {
    static DATA: OnceLock<Vec<(String, Vec<String>)>> = OnceLock::new();
    DATA.get_or_init(|| {
        let json_str = include_str!("../../valence_registry/extracted/registries.json");
        let json: serde_json::Value =
            serde_json::from_str(json_str).expect("failed to parse registries.json");
        let mut result = vec![];
        if let Some(obj) = json.as_object() {
            for (reg_name, reg_value) in obj {
                let mut entry_names: Vec<String> = reg_value
                    .get("entries")
                    .and_then(|e| e.as_object())
                    .map(|entries| entries.keys().cloned().collect())
                    .unwrap_or_default();
                entry_names.sort();
                result.push((reg_name.clone(), entry_names));
            }
        }
        result.sort_by(|a, b| a.0.cmp(&b.0));
        result
    })
}

fn get_tags_data() -> &'static ConfigRegistryMap {
    static TAGS_DATA: OnceLock<ConfigRegistryMap> = OnceLock::new();
    TAGS_DATA.get_or_init(|| {
        let tags_str = include_str!("../../valence_registry/extracted/tags.json");
        let tags_json: serde_json::Value =
            serde_json::from_str(tags_str).expect("failed to parse tags.json");

        let registries_str = include_str!("../../valence_registry/extracted/registries.json");
        let registries_json: serde_json::Value =
            serde_json::from_str(registries_str).expect("failed to parse registries.json");

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

        let mut result = ConfigRegistryMap::new();

        if let Some(registries) = tags_json.as_object() {
            for (registry_name, tags_value) in registries {
                let full_reg_name = if registry_name.contains(':') {
                    registry_name.clone()
                } else {
                    format!("minecraft:{registry_name}")
                };

                let reg_ident: Ident<String> = Ident::new(full_reg_name.clone())
                    .unwrap_or_else(|_| Ident::new("minecraft:unknown".to_owned()).unwrap())
                    .into();

                let id_map = registry_id_map.get(&full_reg_name);

                let mut tag_map = BTreeMap::new();

                if let Some(tags_obj) = tags_value.as_object() {
                    for (tag_name, entries_value) in tags_obj {
                        let tag_ident: Ident<String> = Ident::new(tag_name.clone())
                            .unwrap_or_else(|_| Ident::new("minecraft:unknown".to_owned()).unwrap())
                            .into();
                        let entries: Vec<VarInt> = entries_value
                            .as_array()
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| {
                                        let s = v.as_str()?;
                                        let entry_name = s.strip_prefix('#').unwrap_or(s);
                                        if let Some(map) = id_map {
                                            map.get(entry_name).map(|&id| VarInt(id))
                                        } else {
                                            None
                                        }
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        tag_map.insert(tag_ident, entries);
                    }
                }

                if !tag_map.is_empty() {
                    result.insert(reg_ident, tag_map);
                }
            }
        }

        info!("loaded {} tag registries from tags.json", result.len());

        // Merge jar dynamic tags (for registries not in tags.json)
        let jar_tags = get_jar_dynamic_tags();
        let jar_regs = get_jar_dynamic_registries();

        for (full_reg_name, tag_map_raw) in jar_tags {
            let reg_ident: Ident<String> = Ident::new(full_reg_name.clone())
                .unwrap_or_else(|_| Ident::new("minecraft:unknown".to_owned()).unwrap())
                .into();

            // Build protocol ID map from jar entries (sorted order = protocol ID)
            let mut id_map = std::collections::HashMap::new();
            if let Some(entries) = jar_regs.get(full_reg_name.as_str()) {
                for (idx, (name, _)) in entries.iter().enumerate() {
                    id_map.insert(name.clone(), idx as i32);
                }
            }
            // Also check registries.json for protocol IDs
            if let Some(map) = registry_id_map.get(full_reg_name.as_str()) {
                for (name, id) in map {
                    id_map.entry(name.clone()).or_insert(*id);
                }
            }

            let mut tag_map = BTreeMap::new();
            for (tag_name, entry_names) in tag_map_raw {
                let tag_ident: Ident<String> = Ident::new(tag_name.clone())
                    .unwrap_or_else(|_| Ident::new("minecraft:unknown".to_owned()).unwrap())
                    .into();
                let entries: Vec<VarInt> = entry_names
                    .iter()
                    .filter_map(|s| {
                        let entry_name = s.strip_prefix('#').unwrap_or(s);
                        id_map.get(entry_name).map(|&id| VarInt(id))
                    })
                    .collect();
                if !entries.is_empty() {
                    tag_map.insert(tag_ident, entries);
                }
            }

            if !tag_map.is_empty() {
                result.entry(reg_ident).or_default().extend(tag_map);
            }
        }

        info!("loaded {} tag registries total", result.len());
        result
    })
}

async fn handle_configuration(io: &mut PacketIo) -> anyhow::Result<()> {
    // 1. Receive client information
    //    The client may send Plugin Messages (e.g. minecraft:brand) before Client Information.
    //    Peek at the packet ID and handle accordingly.
    loop {
        let packet_id = io.recv_packet_id().await?;
        match packet_id {
            0 => {
                // Client Information (Configuration ID 0)
                let _client_info: ConfigClientInformationC2s = io.decode_frame()?;
                trace!("received client information in configuration phase");
                break;
            }
            2 => {
                // Plugin Message / custom_payload (Configuration ID 2)
                let msg: ConfigCustomPayloadC2s = io.decode_frame()?;
                trace!(
                    "skipped plugin message in configuration phase (channel: {:?})",
                    msg.channel
                );
            }
            other => {
                bail!("unexpected packet ID {other} in configuration phase, expected 0 (Client Information) or 2 (Plugin Message)");
            }
        }
    }

    // 2. Send known packs - declare minecraft:core so client uses built-in data
    use valence_server::protocol::packets::configuration::config_select_known_packs_s2c::KnownPack as S2cKnownPack;
    let core_pack = S2cKnownPack {
        namespace: "minecraft",
        id: "core",
        version: "26.2",
    };
    io.send_packet(&ConfigSelectKnownPacksS2c {
        packs: Cow::Owned(vec![core_pack]),
    })
    .await?;

    // 3. Receive client's known packs response
    let known_packs: ConfigSelectKnownPacksC2s = io.recv_packet().await?;

    let core_known = known_packs.packs.iter().any(|p| {
        p.namespace == "minecraft" && p.id == "core" && p.version == "26.2"
    });

    // 4. Send registry data
    //    When both sides know minecraft:core, the client has built-in data for
    //    all core registries. We still need to send ConfigRegistryData for each
    //    registry the client expects, but with has_data=false for entries that
    //    exist in the core pack. For dynamic registries not in core, we must
    //    send actual data.
    send_registry_data(io, core_known).await?;

    // 5. Send tags (damage_type tags required for Finish Configuration)
    let tags = get_tags_data();
    io.send_packet(&ConfigUpdateTagsS2c {
        groups: Cow::Borrowed(tags),
    })
    .await?;

    // 6. Send brand
    let brand = "Mili-rust";
    let mut brand_bytes = Vec::new();
    VarInt(brand.len() as i32).encode(&mut brand_bytes)?;
    brand_bytes.extend_from_slice(brand.as_bytes());
    io.send_packet(&ConfigCustomPayloadS2c {
        channel: ident!("minecraft:brand").into(),
        data: valence_protocol::Bounded(valence_protocol::RawBytes(&brand_bytes)),
    })
    .await?;

    // 6.5. Send enabled features (minecraft:vanilla required for MC 26.2)
    io.send_packet(&ConfigUpdateEnabledFeaturesS2c {
        features: Cow::Borrowed(&["minecraft:vanilla"]),
    })
    .await?;

    // 7. Send finish configuration
    io.send_packet(&ConfigFinishConfigurationS2c).await?;

    // 8. Receive client's finish configuration ack
    let _finish_ack: ConfigFinishConfigurationC2s = io.recv_packet().await?;
    trace!("configuration phase complete");

    Ok(())
}

/// Accepts new connections to the server as they occur.
pub(super) async fn do_accept_loop(shared: SharedNetworkState) {
    let listener = match TcpListener::bind(shared.0.address).await {
        Ok(listener) => listener,
        Err(e) => {
            error!("failed to start TCP listener: {e}");
            return;
        }
    };

    let timeout = Duration::from_secs(5);

    loop {
        match shared.0.connection_sema.clone().acquire_owned().await {
            Ok(permit) => match listener.accept().await {
                Ok((stream, remote_addr)) => {
                    let shared = shared.clone();

                    tokio::spawn(async move {
                        if let Err(e) = tokio::time::timeout(
                            timeout,
                            handle_connection(shared, stream, remote_addr),
                        )
                        .await
                        {
                            warn!("initial connection timed out: {e}");
                        }

                        drop(permit);
                    });
                }
                Err(e) => {
                    error!("failed to accept incoming connection: {e}");
                }
            },
            // Closed semaphore indicates server shutdown.
            Err(_) => return,
        }
    }
}

async fn handle_connection(
    shared: SharedNetworkState,
    mut stream: TcpStream,
    remote_addr: SocketAddr,
) {
    trace!("handling connection");

    if let Err(e) = stream.set_nodelay(true) {
        error!("failed to set TCP_NODELAY: {e}");
    }

    match try_handle_legacy_ping(&shared, &mut stream, remote_addr).await {
        Ok(true) => return, // Legacy ping succeeded.
        Ok(false) => {}     // No legacy ping.
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {}
        Err(e) => {
            warn!("legacy ping ended with error: {e:#}");
        }
    }

    let io = PacketIo::new(stream, PacketEncoder::new(), PacketDecoder::new());

    if let Err(e) = handle_handshake(shared, io, remote_addr).await {
        // EOF can happen if the client disconnects while joining, which isn't
        // very erroneous.
        if let Some(e) = e.downcast_ref::<io::Error>() {
            if e.kind() == io::ErrorKind::UnexpectedEof {
                return;
            }
        }
        warn!("connection ended with error: {e:#}");
    }
}

/// Basic information about a client, provided at the beginning of the
/// connection
#[derive(Default, Debug)]
pub struct HandshakeData {
    /// The protocol version of the client.
    pub protocol_version: i32,
    /// The address that the client used to connect.
    pub server_address: String,
    /// The port that the client used to connect.
    pub server_port: u16,
}

async fn handle_handshake(
    shared: SharedNetworkState,
    mut io: PacketIo,
    remote_addr: SocketAddr,
) -> anyhow::Result<()> {
    let handshake = io.recv_packet::<HandshakeC2s>().await?;

    let next_state = handshake.next_state;

    let handshake = HandshakeData {
        protocol_version: handshake.protocol_version.0,
        server_address: handshake.server_address.0.to_owned(),
        server_port: handshake.server_port,
    };

    // TODO: this is borked.
    ensure!(
        shared.0.connection_mode == ConnectionMode::BungeeCord
            || handshake.server_address.encode_utf16().count() <= 255,
        "handshake server address is too long"
    );

    match next_state {
        HandshakeNextState::Status => handle_status(shared, io, remote_addr, handshake)
            .await
            .context("handling status"),
        HandshakeNextState::Login => {
            match handle_login(&shared, &mut io, remote_addr, handshake)
                .await
                .context("handling login")?
            {
                Some((info, cleanup)) => {
                    let client = io.into_client_args(
                        info,
                        shared.0.incoming_byte_limit,
                        shared.0.outgoing_byte_limit,
                        cleanup,
                    );

                    let _ = shared.0.new_clients_send.send_async(client).await;

                    Ok(())
                }
                None => Ok(()),
            }
        }
    }
}

async fn handle_status(
    shared: SharedNetworkState,
    mut io: PacketIo,
    remote_addr: SocketAddr,
    handshake: HandshakeData,
) -> anyhow::Result<()> {
    io.recv_packet::<QueryRequestC2s>().await?;

    match shared
        .0
        .callbacks
        .inner
        .server_list_ping(&shared, remote_addr, &handshake)
        .await
    {
        ServerListPing::Respond {
            online_players,
            max_players,
            player_sample,
            mut description,
            favicon_png,
            version_name,
            protocol,
        } => {
            // For pre-1.16 clients, replace all webcolors with their closest
            // normal colors Because webcolor support was only
            // added at 1.16.
            if handshake.protocol_version < 735 {
                fn fallback_webcolors(txt: &mut Text) {
                    if let Some(Color::Rgb(color)) = txt.color {
                        txt.color = Some(Color::Named(color.to_named_lossy()));
                    }
                    for child in &mut txt.extra {
                        fallback_webcolors(child);
                    }
                }

                fallback_webcolors(&mut description);
            }

            let mut json = json!({
                "version": {
                    "name": version_name,
                    "protocol": protocol,
                },
                "players": {
                    "online": online_players,
                    "max": max_players,
                    "sample": player_sample,
                },
                "description": description,
            });

            if !favicon_png.is_empty() {
                let mut buf = "data:image/png;base64,".to_owned();
                BASE64_STANDARD.encode_string(favicon_png, &mut buf);
                json["favicon"] = Value::String(buf);
            }

            io.send_packet(&QueryResponseS2c {
                json: &json.to_string(),
            })
            .await?;
        }
        ServerListPing::Ignore => return Ok(()),
    }

    let QueryPingC2s { payload } = io.recv_packet().await?;

    io.send_packet(&QueryPongS2c { payload }).await?;

    Ok(())
}

/// Handle the login process and return the new client's data if successful.
async fn handle_login(
    shared: &SharedNetworkState,
    io: &mut PacketIo,
    remote_addr: SocketAddr,
    handshake: HandshakeData,
) -> anyhow::Result<Option<(NewClientInfo, CleanupOnDrop)>> {
    if handshake.protocol_version != PROTOCOL_VERSION {
        io.send_packet(&LoginDisconnectS2c {
            // TODO: use correct translation key.
            reason: format!("Mismatched Minecraft version (server is on {MINECRAFT_VERSION})")
                .color(Color::RED)
                .into(),
        })
        .await?;

        return Ok(None);
    }

    let LoginHelloC2s {
        username,
        .. // TODO: profile_id
    } = io.recv_packet().await?;

    let username = username.0.to_owned();

    // Step 1: Setup compression FIRST (before encryption, matching ferrumc order)
    if shared.0.threshold.0 > 0 {
        io.send_packet(&LoginCompressionS2c {
            threshold: shared.0.threshold.0.into(),
        })
        .await?;

        io.set_compression(shared.0.threshold);
    }

    // Step 2: Authentication + encryption (after compression)
    let info = match shared.connection_mode() {
        ConnectionMode::Online { .. } => login_online(shared, io, remote_addr, username).await?,
        ConnectionMode::Offline => login_offline(remote_addr, username)?,
        ConnectionMode::BungeeCord => {
            login_bungeecord(remote_addr, &handshake.server_address, username)?
        }
        ConnectionMode::Velocity { secret } => login_velocity(io, username, secret).await?,
    };

    let cleanup = match shared.0.callbacks.inner.login(shared, &info).await {
        Ok(f) => CleanupOnDrop(Some(f)),
        Err(reason) => {
            info!("disconnect at login: \"{reason}\"");
            io.send_packet(&LoginDisconnectS2c {
                reason: reason.into(),
            })
            .await?;
            return Ok(None);
        }
    };

    io.send_packet(&LoginSuccessS2c {
        uuid: info.uuid,
        username: info.username.as_str().into(),
        properties: Default::default(),
        session_id: info.uuid,
    })
    .await?;

    // Debug: encode LoginSuccessS2c to bytes and log for protocol analysis
    let mut debug_buf = Vec::new();
    let debug_pkt = LoginSuccessS2c {
        uuid: info.uuid,
        username: (&info.username[..]).into(),
        properties: Default::default(),
        session_id: info.uuid,
    };
    <LoginSuccessS2c as valence_protocol::Packet>::encode_with_id(&debug_pkt, &mut debug_buf).ok();
    error!(
        "LoginSuccessS2c raw bytes (id={} len={}): {:02x?}",
        <LoginSuccessS2c as valence_protocol::Packet>::ID,
        debug_buf.len(),
        &debug_buf[..],
    );

    // Wait for LoginAcknowledged (required by modern MC clients)
    let _login_ack: LoginAcknowledgedC2s = io.recv_packet().await?;
    trace!("received login acknowledged, entering configuration phase");

    // Handle Configuration phase
    handle_configuration(io).await?;

    Ok(Some((info, cleanup)))
}

/// Login procedure for online mode.
async fn login_online(
    shared: &SharedNetworkState,
    io: &mut PacketIo,
    remote_addr: SocketAddr,
    username: String,
) -> anyhow::Result<NewClientInfo> {
    let my_verify_token: [u8; 16] = rand::random();

    io.send_packet(&LoginHelloS2c {
        server_id: "".into(), // Always empty
        public_key: &shared.0.public_key_der,
        verify_token: &my_verify_token,
        should_authenticate: matches!(shared.connection_mode(), ConnectionMode::Online { .. }),
    })
    .await?;

    let LoginKeyC2s {
        shared_secret,
        verify_token: encrypted_verify_token,
    } = io.recv_packet().await?;

    let shared_secret = shared
        .0
        .rsa_key
        .decrypt(Pkcs1v15Encrypt, shared_secret)
        .context("failed to decrypt shared secret")?;

    let verify_token = shared
        .0
        .rsa_key
        .decrypt(Pkcs1v15Encrypt, encrypted_verify_token)
        .context("failed to decrypt verify token")?;

    ensure!(
        my_verify_token.as_slice() == verify_token,
        "verify tokens do not match"
    );

    let crypt_key: [u8; 16] = shared_secret
        .as_slice()
        .try_into()
        .context("shared secret has the wrong length")?;

    io.enable_encryption(&crypt_key);

    let hash = Sha1::new()
        .chain(&shared_secret)
        .chain(&shared.0.public_key_der)
        .finalize();

    let url = shared
        .0
        .callbacks
        .inner
        .session_server(
            shared,
            username.as_str(),
            &auth_digest(&hash),
            &remote_addr.ip(),
        )
        .await;

    let resp = shared.0.http_client.get(url).send().await?;

    match resp.status() {
        StatusCode::OK => {}
        StatusCode::NO_CONTENT => {
            let reason = Text::translate(keys::MULTIPLAYER_DISCONNECT_UNVERIFIED_USERNAME, []);
            io.send_packet(&LoginDisconnectS2c {
                reason: reason.into(),
            })
            .await?;
            bail!("session server could not verify username");
        }
        status => {
            bail!("session server GET request failed (status code {status})");
        }
    }

    #[derive(Deserialize)]
    struct GameProfile {
        id: Uuid,
        name: String,
        properties: Vec<Property>,
    }

    let profile: GameProfile = resp.json().await.context("parsing game profile")?;

    ensure!(profile.name == username, "usernames do not match");

    Ok(NewClientInfo {
        uuid: profile.id,
        username,
        ip: remote_addr.ip(),
        properties: Properties(profile.properties),
    })
}

fn auth_digest(bytes: &[u8]) -> String {
    BigInt::from_signed_bytes_be(bytes).to_str_radix(16)
}

fn offline_uuid(username: &str) -> anyhow::Result<Uuid> {
    Uuid::from_slice(&Sha256::digest(username)[..16]).map_err(Into::into)
}

/// Login procedure for offline mode.
fn login_offline(remote_addr: SocketAddr, username: String) -> anyhow::Result<NewClientInfo> {
    Ok(NewClientInfo {
        // Derive the client's UUID from a hash of their username.
        uuid: offline_uuid(username.as_str())?,
        username,
        properties: Default::default(),
        ip: remote_addr.ip(),
    })
}

/// Login procedure for `BungeeCord`.
fn login_bungeecord(
    remote_addr: SocketAddr,
    server_address: &str,
    username: String,
) -> anyhow::Result<NewClientInfo> {
    // Get data from server_address field of the handshake
    let data = server_address.split('\0').take(4).collect::<Vec<_>>();

    // Ip of player, only given if ip_forward on bungee is true
    let ip = match data.get(1) {
        Some(ip) => ip.parse()?,
        None => remote_addr.ip(),
    };

    // Uuid of player, only given if ip_forward on bungee is true
    let uuid = match data.get(2) {
        Some(uuid) => uuid.parse()?,
        None => offline_uuid(username.as_str())?,
    };

    // Read properties and get textures
    // Properties of player's game profile, only given if ip_forward and online_mode
    // on bungee both are true
    let properties: Vec<Property> = match data.get(3) {
        Some(properties) => serde_json::from_str(properties)
            .context("failed to parse BungeeCord player properties")?,
        None => vec![],
    };

    Ok(NewClientInfo {
        uuid,
        username,
        properties: Properties(properties),
        ip,
    })
}

/// Login procedure for Velocity.
async fn login_velocity(
    io: &mut PacketIo,
    username: String,
    velocity_secret: &str,
) -> anyhow::Result<NewClientInfo> {
    const VELOCITY_MIN_SUPPORTED_VERSION: u8 = 1;
    const VELOCITY_MODERN_FORWARDING_WITH_KEY_V2: i32 = 3;

    let message_id: i32 = 0; // TODO: make this random?

    // Send Player Info Request into the Plugin Channel
    io.send_packet(&LoginQueryRequestS2c {
        message_id: VarInt(message_id),
        channel: ident!("velocity:player_info").into(),
        data: RawBytes(&[VELOCITY_MIN_SUPPORTED_VERSION]).into(),
    })
    .await?;

    // Get Response
    let plugin_response: LoginQueryResponseC2s = io.recv_packet().await?;

    ensure!(
        plugin_response.message_id.0 == message_id,
        "mismatched plugin response ID (got {}, expected {message_id})",
        plugin_response.message_id.0,
    );

    let data = plugin_response
        .data
        .context("missing plugin response data")?
        .0;

    ensure!(data.len() >= 32, "invalid plugin response data length");
    let (signature, mut data_without_signature) = data.split_at(32);

    // Verify signature
    let mut mac = Hmac::<Sha256>::new_from_slice(velocity_secret.as_bytes())?;
    Mac::update(&mut mac, data_without_signature);
    mac.verify_slice(signature)?;

    // Check Velocity version
    let version = VarInt::decode(&mut data_without_signature)
        .context("failed to decode velocity version")?
        .0;

    // Get client address
    let remote_addr = String::decode(&mut data_without_signature)?.parse()?;

    // Get UUID
    let uuid = Uuid::decode(&mut data_without_signature)?;

    // Get username and validate
    ensure!(
        username == <&str>::decode(&mut data_without_signature)?,
        "mismatched usernames"
    );

    // Read game profile properties
    let properties = Vec::<Property>::decode(&mut data_without_signature)
        .context("decoding velocity game profile properties")?;

    if version >= VELOCITY_MODERN_FORWARDING_WITH_KEY_V2 {
        // TODO
    }

    Ok(NewClientInfo {
        uuid,
        username,
        properties: Properties(properties),
        ip: remote_addr,
    })
}

#[cfg(test)]
mod tests {
    use sha1::Digest;

    use super::*;

    #[test]
    fn auth_digest_usernames() {
        assert_eq!(
            auth_digest(&Sha1::digest("Notch")),
            "4ed1f46bbe04bc756bcb17c0c7ce3e4632f06a48"
        );
        assert_eq!(
            auth_digest(&Sha1::digest("jeb_")),
            "-7c9d5b0044c130109a5d7b5fb5c317c02b4e28c1"
        );
        assert_eq!(
            auth_digest(&Sha1::digest("simon")),
            "88e16a1019277b15d58faf0541e11910eb756f6"
        );
    }
}