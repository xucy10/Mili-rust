@echo off
chcp 65001 >nul
title Mili-rust Server

echo ========================================
echo    Mili-rust Minecraft Server
echo    版本: 1.20.1
echo    端口: 25565
echo ========================================
echo.
echo 用 Minecraft 1.20.1 连接 localhost
echo 按 Ctrl+C 停止服务器
echo.

cargo run --release
pause
