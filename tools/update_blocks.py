#!/usr/bin/env python3
"""
更新 blocks.json 添加 26.2 版本的新方块
"""
import json
import os

# 26.2 新增的方块定义
NEW_BLOCKS_26_2 = [
    # Cinnabar 系列
    {"name": "cinnabar", "hardness": 1.5, "resistance": 6.0, "luminance": 0, "tool": "pickaxe", "requires_tool": False},
    {"name": "polished_cinnabar", "hardness": 1.5, "resistance": 6.0, "luminance": 0, "tool": "pickaxe", "requires_tool": False},
    {"name": "cinnabar_bricks", "hardness": 1.5, "resistance": 6.0, "luminance": 0, "tool": "pickaxe", "requires_tool": False},
    {"name": "chiseled_cinnabar", "hardness": 1.5, "resistance": 6.0, "luminance": 0, "tool": "pickaxe", "requires_tool": False},
    {"name": "cinnabar_stairs", "hardness": 1.5, "resistance": 6.0, "luminance": 0, "tool": "pickaxe", "requires_tool": False},
    {"name": "cinnabar_slab", "hardness": 1.5, "resistance": 6.0, "luminance": 0, "tool": "pickaxe", "requires_tool": False},
    {"name": "cinnabar_wall", "hardness": 1.5, "resistance": 6.0, "luminance": 0, "tool": "pickaxe", "requires_tool": False},
    
    # Sulfur 系列
    {"name": "sulfur", "hardness": 1.5, "resistance": 6.0, "luminance": 0, "tool": "pickaxe", "requires_tool": False},
    {"name": "polished_sulfur", "hardness": 1.5, "resistance": 6.0, "luminance": 0, "tool": "pickaxe", "requires_tool": False},
    {"name": "sulfur_bricks", "hardness": 1.5, "resistance": 6.0, "luminance": 0, "tool": "pickaxe", "requires_tool": False},
    {"name": "chiseled_sulfur", "hardness": 1.5, "resistance": 6.0, "luminance": 0, "tool": "pickaxe", "requires_tool": False},
    {"name": "sulfur_stairs", "hardness": 1.5, "resistance": 6.0, "luminance": 0, "tool": "pickaxe", "requires_tool": False},
    {"name": "sulfur_slab", "hardness": 1.5, "resistance": 6.0, "luminance": 0, "tool": "pickaxe", "requires_tool": False},
    {"name": "sulfur_wall", "hardness": 1.5, "resistance": 6.0, "luminance": 0, "tool": "pickaxe", "requires_tool": False},
    
    # 特殊方块
    {"name": "potent_sulfur", "hardness": 0.5, "resistance": 0.5, "luminance": 0, "tool": "pickaxe", "requires_tool": False},
    {"name": "sulfur_spike", "hardness": 0.5, "resistance": 0.5, "luminance": 0, "tool": "pickaxe", "requires_tool": False},
]

def get_max_state_id(blocks):
    """获取当前最大方块状态ID"""
    max_id = 0
    for block in blocks.get("blocks", []):
        for state in block.get("states", []):
            if state["id"] > max_id:
                max_id = state["id"]
    return max_id

def create_block_entry(block_def, start_id):
    """创建一个方块条目"""
    # 创建默认状态
    default_state_id = start_id
    states = [{"id": default_state_id, "default": True}]
    
    return {
        "id": block_def["id"],
        "ident": f"minecraft:{block_def['name']}",
        "name": block_def["name"],
        "hardness": block_def["hardness"],
        "resistance": block_def["resistance"],
        "luminance": block_def["luminance"],
        "tool": block_def.get("tool", None),
        "requires_tool": block_def.get("requires_tool", False),
        "transparent": False,
        "liquid": False,
        "solid": True,
        "replaceable": False,
        "blocks_motion": True,
        "states": states
    }

def main():
    blocks_path = "crates/valence_generated/extracted/blocks.json"
    
    # 读取现有数据
    with open(blocks_path, 'r') as f:
        data = json.load(f)
    
    # 检查是否已经有这些方块
    existing_names = set()
    for block in data.get("blocks", []):
        existing_names.add(block.get("name", ""))
    
    # 获取最大ID
    max_id = len(data.get("blocks", []))
    max_state_id = get_max_state_id(data)
    
    print(f"当前方块数: {max_id}")
    print(f"当前最大状态ID: {max_state_id}")
    
    # 添加新方块
    added = 0
    for block_def in NEW_BLOCKS_26_2:
        if block_def["name"] not in existing_names:
            block_def["id"] = max_id
            block_entry = create_block_entry(block_def, max_state_id + 1)
            data["blocks"].append(block_entry)
            max_id += 1
            max_state_id += 1
            added += 1
            print(f"  添加: {block_def['name']} (ID: {block_entry['id']})")
    
    # 写回文件
    with open(blocks_path, 'w') as f:
        json.dump(data, f, indent=2)
    
    print(f"\n添加了 {added} 个新方块")
    print(f"总方块数: {len(data['blocks'])}")

if __name__ == "__main__":
    os.chdir(r"E:\Program Files\Tencent\AndrowsData\Mili-rust")
    main()
