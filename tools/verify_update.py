#!/usr/bin/env python3
"""
验证更新后的数据文件
"""
import json
import os

def check_file(filepath, expected_min_count=None, label=""):
    """检查文件是否存在并统计数量"""
    if not os.path.exists(filepath):
        print(f"  [ERROR] {label} 文件不存在: {filepath}")
        return False
    
    with open(filepath, 'r') as f:
        data = json.load(f)
    
    if isinstance(data, list):
        count = len(data)
    elif isinstance(data, dict):
        # 根据结构确定计数方式
        if "blocks" in data:
            count = len(data["blocks"])
        elif "entities" in data:
            count = len(data["entities"])
        elif "block_entity_types" in data:
            count = len(data.get("blocks", []))
        else:
            count = len(data)
    
    print(f"  [OK] {label}: {count} 条记录")
    
    if expected_min_count and count < expected_min_count:
        print(f"    [WARNING] 数量少于预期 ({expected_min_count})")
    
    return True

def main():
    print("=" * 50)
    print("Minecraft 26.2 数据验证")
    print("=" * 50)
    print()
    
    os.chdir(r"E:\Program Files\Tencent\AndrowsData\Mili-rust")
    
    print("检查数据文件:")
    check_file("crates/valence_generated/extracted/blocks.json", label="方块")
    check_file("crates/valence_generated/extracted/items.json", label="物品")
    check_file("crates/valence_generated/extracted/entities.json", label="实体")
    check_file("crates/valence_generated/extracted/sounds.json", label="声音")
    check_file("crates/valence_generated/extracted/attributes.json", label="属性")
    check_file("crates/valence_generated/extracted/effects.json", label="效果")
    check_file("crates/valence_generated/extracted/packets.json", label="数据包")
    
    print()
    print("检查协议版本:")
    with open("crates/valence_protocol/src/lib.rs", 'r') as f:
        content = f.read()
        if "PROTOCOL_VERSION: i32 = 776" in content:
            print("  [OK] PROTOCOL_VERSION = 776")
        else:
            print("  [ERROR] PROTOCOL_VERSION 未更新")
        
        if 'MINECRAFT_VERSION: &str = "26.2"' in content:
            print("  [OK] MINECRAFT_VERSION = 26.2")
        else:
            print("  [ERROR] MINECRAFT_VERSION 未更新")
    
    print()
    print("=" * 50)
    print("验证完成!")
    print("=" * 50)

if __name__ == "__main__":
    main()
