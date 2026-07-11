@echo off
chcp 65001 >nul
title Mili-rust Server

:: 直接运行 vanilla_demo 示例
echo 启动 Mili-rust 服务器...
echo 用 Minecraft 1.20.1 连接 localhost
echo 按 Ctrl+C 停止
echo.
cargo run --release -- example vanilla_demo
pause
