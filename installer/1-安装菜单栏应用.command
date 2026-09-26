#!/bin/zsh
# 安装 OpenCode Stats 菜单栏用量应用
set -u
cd "$(dirname "$0")" || exit 1

SRC="$PWD/OpenCode Stats.app"
DEST="/Applications/OpenCode Stats.app"

if [[ ! -d "$SRC" ]]; then
  echo "✘ 未找到 OpenCode Stats.app（请从 DMG 内直接运行本脚本，不要先复制到别处）"
  read -r "?按回车关闭..."; exit 1
fi

echo "==> 正在安装到 /Applications ..."
if [[ -w /Applications ]]; then
  rm -rf "$DEST"
  cp -R "$SRC" "$DEST" || { echo "✘ 拷贝失败"; read -r "?按回车关闭..."; exit 1; }
else
  osascript -e "do shell script \"rm -rf '$DEST' && cp -R '$SRC' '$DEST'\" with administrator privileges" \
    || { echo "✘ 拷贝失败（已取消授权）"; read -r "?按回车关闭..."; exit 1; }
fi

echo "==> 去除隔离属性（解决“无法打开/已损坏”提示）..."
xattr -dr com.apple.quarantine "$DEST" 2>/dev/null
codesign --force --deep --sign - "$DEST" 2>/dev/null

echo "==> 启动应用（图标出现在屏幕右上角菜单栏）..."
open -a "$DEST" 2>/dev/null || open "$DEST"

DB="$HOME/.local/share/opencode/opencode.db"
if [[ -f "$DB" ]]; then
  echo "✔ 安装完成，已检测到 opencode 数据库: $DB"
else
  echo "⚠ 未检测到 opencode 数据库（$DB）。"
  echo "  应用会显示“未找到数据库”，安装 opencode 并跑过会话后即可自动统计。"
fi

echo
echo "完成。若首次打开仍提示“无法验证开发者”，右键点按应用 → 打开 → 打开。"
read -r "?按回车关闭..."
