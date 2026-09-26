#!/bin/zsh
# 安装 opencode 动态上下文裁剪插件 (opencode-dynamic-context-pruning)
set -u

find_opencode() {
  local c
  for c in opencode "$HOME/.opencode/bin/opencode" "$HOME/.local/bin/opencode" "/usr/local/bin/opencode" "/opt/homebrew/bin/opencode"; do
    if command -v "$c" >/dev/null 2>&1; then command -v "$c"; return 0; fi
    if [[ -x "$c" ]]; then echo "$c"; return 0; fi
  done
  return 1
}

OC="$(find_opencode)" || OC=""

if [[ -z "$OC" ]]; then
  echo "✘ 未找到 opencode 可执行文件。"
  echo "  请先安装 opencode（https://opencode.ai），或手动执行："
  echo "      opencode plugin -g @tarquinen/opencode-dcp@latest"
  echo
  read -r "?按回车关闭..."; exit 1
fi

echo "==> 使用: $OC"
echo "==> 安装全局插件 @tarquinen/opencode-dcp@latest ..."
if "$OC" plugin -g "@tarquinen/opencode-dcp@latest"; then
  echo
  echo "✔ 插件已安装（全局）。重启 opencode 后生效，可用命令: dcp-compress 等。"
  echo "  插件会在上下文接近上限时自动裁剪/压缩，缓解长会话爆上下文。"
else
  echo "⚠ 自动安装失败，请手动执行:  opencode plugin -g @tarquinen/opencode-dcp@latest"
fi

echo
read -r "?按回车关闭..."
