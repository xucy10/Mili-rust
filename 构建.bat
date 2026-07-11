@echo off
chcp 65001 >nul
title Mili-rust 构建工具

echo ========================================
echo    Mili-rust 构建工具
echo ========================================
echo.
echo [1] 构建 Release 版本 (推荐)
echo [2] 构建 Dev 版本 (更快)
echo [3] 清理构建产物
echo [4] 运行服务器
echo [0] 退出
echo.

set /p choice=请选择: 

if "%choice%"=="1" goto release
if "%choice%"=="2" goto dev
if "%choice%"=="3" goto clean
if "%choice%"=="4" goto run
if "%choice%"=="0" exit

:release
echo.
echo 构建 Release 版本...
echo 这可能需要几分钟，首次编译较慢...
echo.
cargo build --release
if %errorlevel% neq 0 (
    echo.
    echo 构建失败！
    pause
    exit /b 1
)
echo.
echo 构建成功！
echo 可执行文件位置: target\release\mili-rust.exe
echo.
echo 是否立即运行？(Y/N)
set /p runnow=
if /i "%runnow%"=="Y" goto run
pause
goto end

:dev
echo.
echo 构建 Dev 版本...
cargo build
if %errorlevel% neq 0 (
    echo.
    echo 构建失败！
    pause
    exit /b 1
)
echo.
echo 构建成功！
echo 可执行文件位置: target\debug\mili-rust.exe
pause
goto end

:clean
echo.
echo 清理构建产物...
cargo clean
echo.
echo 清理完成！
pause
goto end

:run
echo.
echo 启动服务器...
echo 用 Minecraft 1.20.1 连接 localhost
echo.
if exist "target\release\mili-rust.exe" (
    target\release\mili-rust.exe
) else if exist "target\debug\mili-rust.exe" (
    target\debug\mili-rust.exe
) else (
    echo 未找到可执行文件，请先构建
    cargo run --release
)
goto end

:end
pause
