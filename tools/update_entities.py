#!/usr/bin/env python3
"""
更新 entities.json 添加 26.2 版本的新实体
"""
import json
import os

# 26.2 新增的实体定义
NEW_ENTITIES_26_2 = [
    {
        "name": "sulfur_cube",
        "category": "monster",
        "health": 10.0,
        "width": 0.98,
        "height": 0.98,
        "attributes": ["minecraft:follow_range", "minecraft:knockback_resistance", "minecraft:movement_speed", "minecraft:air_drag_modifier"],
        "tracks": True,
        "summonable": True,
        "spawn_egg": True,
    },
]

def main():
    entities_path = "crates/valence_generated/extracted/entities.json"
    
    # 检查文件是否存在
    if not os.path.exists(entities_path):
        # 创建新文件
        data = {"entities": []}
    else:
        # 读取现有数据
        with open(entities_path, 'r') as f:
            data = json.load(f)
    
    # 检查是否已经有这些实体
    existing_names = set()
    for entity in data.get("entities", []):
        existing_names.add(entity.get("name", ""))
    
    # 获取最大ID
    max_id = len(data.get("entities", []))
    
    print(f"当前实体数: {max_id}")
    
    # 添加新实体
    added = 0
    for entity_def in NEW_ENTITIES_26_2:
        if entity_def["name"] not in existing_names:
            entity_entry = {
                "id": max_id,
                "ident": f"minecraft:{entity_def['name']}",
                "name": entity_def["name"],
                "category": entity_def.get("category", "misc"),
                "health": entity_def.get("health", 20.0),
                "width": entity_def.get("width", 0.6),
                "height": entity_def.get("height", 1.8),
                "attributes": entity_def.get("attributes", []),
                "tracks": entity_def.get("tracks", True),
                "summonable": entity_def.get("summonable", False),
                "spawn_egg": entity_def.get("spawn_egg", False),
            }
            data["entities"].append(entity_entry)
            max_id += 1
            added += 1
            print(f"  添加: {entity_def['name']} (ID: {entity_entry['id']})")
    
    # 写回文件
    with open(entities_path, 'w') as f:
        json.dump(data, f, indent=2)
    
    print(f"\n添加了 {added} 个新实体")
    print(f"总实体数: {len(data['entities'])}")

if __name__ == "__main__":
    os.chdir(r"E:\Program Files\Tencent\AndrowsData\Mili-rust")
    main()
