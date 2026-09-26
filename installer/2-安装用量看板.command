#!/bin/zsh
# 安装 opencode 整机用量看板（本地 Web 服务 + 开机自启）
set -u
cd "$(dirname "$0")" || exit 1

BIN_SRC="$PWD/bin/opencode-token-dashboard"
INSTALL_DIR="$HOME/.local/share/opencode-token-dashboard"
BIN="$INSTALL_DIR/opencode-token-dashboard"
LABEL="com.lennylab.opencode-token-dashboard"
PLIST="$HOME/Library/LaunchAgents/$LABEL.plist"
CTRL_DIR="$HOME/.local/bin"
CTRL="$CTRL_DIR/opencode-dashboard"
PORT="${PORT:-8765}"

if [[ ! -f "$BIN_SRC" ]]; then
  echo "✘ 未找到 bin/opencode-token-dashboard（请在 DMG 内直接运行本脚本）"
  read -r "?按回车关闭..."; exit 1
fi

echo "==> 安装程序到 $INSTALL_DIR"
mkdir -p "$INSTALL_DIR" "$CTRL_DIR"
cp -f "$BIN_SRC" "$BIN" && chmod +x "$BIN"
echo "PORT=$PORT" > "$INSTALL_DIR/env"

echo "==> 写入控制命令 $CTRL"
cat > "$CTRL" <<'CTRL_EOF'
#!/bin/zsh
# opencode 用量看板控制命令
set -u
BIN="$HOME/.local/share/opencode-token-dashboard/opencode-token-dashboard"
LABEL="com.lennylab.opencode-token-dashboard"
PLIST="$HOME/Library/LaunchAgents/$LABEL.plist"
[[ -f "$HOME/.local/share/opencode-token-dashboard/env" ]] && . "$HOME/.local/share/opencode-token-dashboard/env"
PORT="${PORT:-8765}"
UID_N="$(id -u)"
URL="http://127.0.0.1:$PORT/"

managed() { launchctl print "gui/$UID_N/$LABEL" >/dev/null 2>&1; }
listening() { lsof -nP -iTCP:"$PORT" -sTCP:LISTEN -t >/dev/null 2>&1; }

start() {
  if listening; then echo "已在运行  $URL"; return 0; fi
  [[ -x "$BIN" ]] || { echo "未安装: $BIN" >&2; return 1; }
  if [[ -f "$PLIST" ]]; then
    managed || launchctl bootstrap "gui/$UID_N" "$PLIST" 2>/dev/null
    launchctl kickstart "gui/$UID_N/$LABEL" 2>/dev/null
  else
    (cd "$HOME" && PORT="$PORT" nohup "$BIN" >/tmp/opencode-dashboard.log 2>&1 &)
  fi
  for i in 1 2 3 4 5 6 7 8; do listening && break; sleep 0.5; done
  listening && echo "已启动: $URL" || { echo "启动失败，日志见 /tmp/opencode-dashboard.log" >&2; return 1; }
}
stop() {
  if managed; then launchctl bootout "gui/$UID_N/$LABEL" 2>/dev/null; echo "已停止并卸载开机自启"; return 0; fi
  if listening; then kill "$(lsof -nP -iTCP:"$PORT" -sTCP:LISTEN -t | head -1)" 2>/dev/null && echo "已停止"; else echo "未在运行"; fi
}
case "${1:-status}" in
  start) start ;;
  stop) stop ;;
  restart) stop; sleep 1; start ;;
  open) start >/dev/null; open "$URL" ;;
  status)
    if listening; then
      if managed; then echo "运行中  $URL  （开机自启已启用）"; else echo "运行中  $URL"; fi
    else
      echo "未运行"
    fi ;;
  uninstall)
    stop >/dev/null 2>&1
    rm -f "$PLIST" "$HOME/.local/bin/opencode-dashboard"
    rm -rf "$HOME/.local/share/opencode-token-dashboard"
    echo "已卸载";;
  *) echo "用法: opencode-dashboard {start|stop|restart|open|status|uninstall}"; exit 1;;
esac
CTRL_EOF
chmod +x "$CTRL"

echo "==> 写入开机自启 LaunchAgent"
cat > "$PLIST" <<PLIST_EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>$LABEL</string>
  <key>ProgramArguments</key>
  <array>
    <string>$BIN</string>
  </array>
  <key>EnvironmentVariables</key>
  <dict>
    <key>PORT</key>
    <string>$PORT</string>
    <key>HOST</key>
    <string>127.0.0.1</string>
  </dict>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <dict>
    <key>SuccessfulExit</key>
    <false/>
  </dict>
  <key>StandardOutPath</key>
  <string>/tmp/opencode-dashboard.log</string>
  <key>StandardErrorPath</key>
  <string>/tmp/opencode-dashboard.log</string>
</dict>
</plist>
PLIST_EOF

UID_N="$(id -u)"
launchctl bootout "gui/$UID_N/$LABEL" 2>/dev/null
launchctl bootstrap "gui/$UID_N" "$PLIST" 2>/dev/null
launchctl kickstart "gui/$UID_N/$LABEL" 2>/dev/null

for i in 1 2 3 4 5 6 7 8 9 10; do
  lsof -nP -iTCP:"$PORT" -sTCP:LISTEN -t >/dev/null 2>&1 && break
  sleep 0.5
done

if lsof -nP -iTCP:"$PORT" -sTCP:LISTEN -t >/dev/null 2>&1; then
  echo "✔ 看板已启动: http://127.0.0.1:$PORT/  （已设为登录后自动运行）"
  command -v open >/dev/null && open "http://127.0.0.1:$PORT/"
else
  echo "⚠ 启动可能失败，可稍后手动执行: opencode-dashboard start"
fi

case ":$PATH:" in
  *":$CTRL_DIR:"*) ;;
  *) echo "提示: 把 $CTRL_DIR 加入 PATH 后可直接使用 opencode-dashboard 命令" ;;
esac

echo
read -r "?按回车关闭..."
