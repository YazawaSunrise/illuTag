@echo off
REM --- illuTag 启动脚本 ---

REM 1. 设置代码页为 UTF-8，防止 Python 和批处理中的中文出现乱码
chcp 65001 > nul

:menu
echo.
echo ===================================
echo  illuTag 图像索引工具 启动器
echo ===================================
echo.
echo  请选择启动模式:
echo.
echo    [1] 完整模式 (默认 - 加载 AI 模型, 支持扫描和搜索)
echo    [2] 搜索专用模式 (--search-only - 不加载模型, 仅搜索)
echo.

set "LAUNCH_OPTIONS="
set /p "mode=请输入选项 (1 或 2，默认为 1): "

if "%mode%"=="1" (
    set "LAUNCH_OPTIONS="
    echo "已选择: 完整模式"
) else if "%mode%"=="2" (
    set "LAUNCH_OPTIONS=--search-only"
    echo "已选择: 搜索专用模式"
) else (
    set "LAUNCH_OPTIONS="
    echo "输入无效或为空，已选择默认值: 完整模式"
)

echo.
echo -----------------------------------
echo 正在激活 Conda 虚拟环境 'illuTag_env'...
echo -----------------------------------
echo.

REM 2. 激活 Conda 虚拟环境
REM    (这假设 'conda' 命令已在你的系统 PATH 中)
call conda activate illuTag_env

if errorlevel 1 (
    echo.
    echo -----------------------------------
    echo 错误: 无法激活 Conda 环境 'illuTag_env'。
    echo.
    echo 请确保:
    echo   1. Anaconda/Miniconda 已安装。
    echo   2. 'illuTag_env' 虚拟环境已正确创建。
    echo   3. 你是从 'Anaconda Prompt' 或已配置 Conda 的终端运行此脚本。
    echo -----------------------------------
    echo.
    pause
    goto :eof
)

echo 环境 'illuTag_env' 激活成功。
echo.
echo -----------------------------------
echo 正在启动 illuTag...
echo (启动参数: %LAUNCH_OPTIONS%)
echo -----------------------------------
echo.

REM 3. 启动 Python 应用程序，并传入选择的启动项
python app.py %LAUNCH_OPTIONS%

echo.
echo -----------------------------------
echo illuTag 应用已关闭。
echo 按任意键退出此窗口...
echo -----------------------------------
pause > nul