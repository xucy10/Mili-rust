#!/usr/bin/env python3
"""
更新 items.json 添加 26.2 版本的新物品
"""
import json
import os

# 26.2 新增的物品定义
NEW_ITEMS_26_2 = [
    # 方块物品（与新方块对应）
    {"name": "cinnabar", "stack_size": 64, "category": "building_blocks"},
    {"name": "polished_cinnabar", "stack_size": 64, "category": "building_blocks"},
    {"name": "cinnabar_bricks", "stack_size": 64, "category": "building_blocks"},
    {"name": "chiseled_cinnabar", "stack_size": 64, "category": "building_blocks"},
    {"name": "cinnabar_stairs", "stack_size": 64, "category": "building_blocks"},
    {"name": "cinnabar_slab", "stack_size": 64, "category": "building_blocks"},
    {"name": "cinnabar_wall", "stack_size": 64, "category": "building_blocks"},
    {"name": "sulfur", "stack_size": 64, "category": "building_blocks"},
    {"name": "polished_sulfur", "stack_size": 64, "category": "building_blocks"},
    {"name": "sulfur_bricks", "stack_size": 64, "category": "building_blocks"},
    {"name": "chiseled_sulfur", "stack_size": 64, "category": "building_blocks"},
    {"name": "sulfur_stairs", "stack_size": 64, "category": "building_blocks"},
    {"name": "sulfur_slab", "stack_size": 64, "category": "building_blocks"},
    {"name": "sulfur_wall", "stack_size": 64, "category": "building_blocks"},
    {"name": "potent_sulfur", "stack_size": 64, "category": "building_blocks"},
    {"name": "sulfur_spike", "stack_size": 64, "category": "building_blocks"},
    
    # 物品
    {"name": "sulfur_cube_bucket", "stack_size": 1, "category": "misc"},
    {"name": "sulfur_cube_spawn_egg", "stack_size": 64, "category": "misc"},
    {"name": "music_disc_bounce", "stack_size": 1, "category": "misc"},
]

def main():
    items_path = "crates/valence_generated/extracted/items.json"
    
    # 读取现有数据
    with open(items_path, 'r') as f:
        data = json.load(f)
    
    # 检查是否已经有这些物品
    existing_names = set()
    for item in data:
        existing_names.add(item.get("name", ""))
    
    # 获取最大ID
    max_id = max(item.get("id", 0) for item in data) + 1
    
    print(f"当前物品数: {len(data)}")
    print(f"当前最大ID: {max_id - 1}")
    
    # 添加新物品
    added = 0
    for item_def in NEW_ITEMS_26_2:
        if item_def["name"] not in existing_names:
            item_entry = {
                "id": max_id,
                "ident": f"minecraft:{item_def['name']}",
                "name": item_def["name"],
                "stack_size": item_def["stack_size"],
                "category": item_def.get("category", "misc")
            }
            data.append(item_entry)
            max_id += 1
            added += 1
            print(f"  添加: {item_def['name']} (ID: {item_entry['id']})")
    
    # 写回文件
    with open(items_path, 'w') as f:
        json.dump(data, f, indent=2)
    
    print(f"\n添加了 {added} 个新物品")
    print(f"总物品数: {len(data)}")

if __name__ == "__main__":
    os.chdir(r"E:\Program Files\Tencent\AndrowsData\Mili-rust")
    main()
