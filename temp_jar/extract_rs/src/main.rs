use std::fs;
use std::io::Read;

fn main() {
    let jar_path = r"E:\Program Files\Tencent\AndrowsData\Mili-rust\server.jar";
    let out_dir = r"E:\Program Files\Tencent\AndrowsData\Mili-rust\crates\valence_registry\extracted";

    let file = fs::File::open(jar_path).expect("failed to open server.jar");
    let mut archive = zip::ZipArchive::new(file).expect("failed to read zip");

    let needed = [
        "cat_variant", "cat_sound_variant",
        "chicken_variant", "chicken_sound_variant",
        "cow_variant", "cow_sound_variant",
        "frog_variant",
        "painting_variant",
        "pig_variant", "pig_sound_variant",
        "wolf_variant", "wolf_sound_variant",
        "zombie_nautilus_variant",
        "banner_pattern",
        "enchantment",
        "damage_type",
        "instrument",
        "dialog",
        "timeline",
    ];

    let mut all_data_dirs = std::collections::BTreeSet::new();
    for i in 0..archive.len() {
        let file = archive.by_index(i).unwrap();
        let name = file.name().to_string();
        if name.starts_with("data/minecraft/") {
            let parts: Vec<&str> = name.split('/').collect();
            if parts.len() >= 3 {
                all_data_dirs.insert(parts[2].to_string());
            }
        }
    }

    println!("All registry directories in server.jar:");
    for d in &all_data_dirs {
        println!("  {d}");
    }

    for reg in &needed {
        let prefix = format!("data/minecraft/{reg}/");
        let mut entries = vec![];
        for i in 0..archive.len() {
            let mut file = archive.by_index(i).unwrap();
            let name = file.name().to_string();
            if name.starts_with(&prefix) && name.ends_with(".json") {
                let entry_id = name.split('/').last().unwrap().replace(".json", "");
                let mut content = String::new();
                file.read_to_string(&mut content).unwrap();
                entries.push((format!("minecraft:{entry_id}"), content));
            }
        }

        if !entries.is_empty() {
            let out_path = format!("{out_dir}/{reg}.json");
            let mut json_obj = serde_json::Map::new();
            for (id, content) in &entries {
                if let Ok(v) = serde_json::from_str(content) {
                    json_obj.insert(id.clone(), v);
                }
            }
            let json_str = serde_json::to_string_pretty(&json_obj).unwrap();
            fs::write(&out_path, &json_str).unwrap();
            println!("Extracted {reg}: {} entries -> {out_path}", entries.len());
        } else {
            println!("MISSING: {reg} - no entries found");
        }
    }

    println!("\nDone!");
}