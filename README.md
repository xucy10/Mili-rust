# Mili-rust Minecraft Server

一个用 Rust 构建的 Minecraft 26.2 服务器框架，基于 Bevy ECS 架构。

## 功能特性

### 核心系统
- ✅ 完整的 Minecraft 26.2 协议支持
- ✅ Bevy ECS 架构
- ✅ 区块管理与网络同步
- ✅ 实体系统与元数据追踪
- ✅ 背包与物品系统
- ✅ 网络加密与压缩

### 原版机制
- ✅ 方块更新传播系统
- ✅ Tick 调度器（随机tick + 计划tick）
- ✅ 红石系统（红石线、火把、中继器、比较器、活塞、红石灯）
- ✅ 漏斗系统（物品传输）
- ✅ 作物生长系统（随机tick + 骨粉加速）
- ✅ 物理引擎（重力、碰撞检测）
- ✅ 生物AI（A*寻路 + 行为树）
- ✅ 村民系统（职业、交易、AI）

### 世界管理
- ✅ Anvil 世界格式读写
- ✅ level.dat 读写
- ✅ 自动保存系统
- ✅ 区块加载/卸载

### 26.2 新特性
- ✅ 新方块：Cinnabar 系列、Sulfur 系列
- ✅ 新实体：Sulfur Cube
- ✅ 新生物群系：Sulfur Caves

## 快速开始

### 前置条件

1. **Rust** - <https://rustup.rs>
2. **Visual Studio Build Tools** - <https://visualstudio.microsoft.com/visual-cpp-build-tools/>
   - 安装时选择 "C++ 桌面开发"

### 构建和运行

```bash
# 构建 Release 版本
cargo build --release

# 运行服务器
./target/release/mili-rust.exe
```

### 分发

构建完成后，`target/release/mili-rust.exe` 可以直接发给其他人运行，无需安装 Rust。

## 使用客户端连接

1. 启动服务器
2. 打开 Minecraft **26.2** 客户端
3. 连接 `localhost`

## 示例程序

```bash
# 运行原版机制演示
cargo run --release -- example vanilla_demo

# 运行其他示例
cargo run --release -- example building
cargo run --release -- example parkour
cargo run --release -- example terrain
```

## 开发

### 添加新方块

编辑 `crates/valence_generated/extracted/blocks.json`

### 添加新物品

编辑 `crates/valence_generated/extracted/items.json`

### 重新生成代码

```bash
cargo build
```

## 项目结构

```text
Mili-rust/
├── crates/
│   ├── valence_protocol/      # 网络协议
│   ├── valence_server/        # 服务器核心
│   ├── valence_entity/        # 实体系统
│   ├── valence_inventory/     # 背包系统
│   ├── valence_generated/     # 代码生成
│   ├── valence_anvil/         # Anvil格式
│   ├── valence_nbt/           # NBT编解码
│   ├── valence_vanilla/       # 原版机制
│   └── valence_world/         # 世界管理
├── examples/                  # 示例程序
├── src/                       # 主程序入口
└── tools/                     # 构建工具
```

## 协议版本

- **Minecraft 版本**: 26.2
- **协议版本**: 776
- **数据版本**: 4903

## 许可证

MIT License
