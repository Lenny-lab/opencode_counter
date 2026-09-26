#!/bin/zsh
# OpenCode Token Dashboard 控制脚本
# 用法: ./dashboard.sh {start|stop|restart|status|open|autostart|no-autostart}

set -u
APP_DIR="${0:A:h}"
BIN="$APP_DIR/bin/opencode-token-dashboard"
PORT="${PORT:-8765}"
LOG="$APP_DIR/logs/dashboard.log"
PIDFILE="$APP_DIR/logs/dashboard.pid"
LABEL="com.putou.opencode-token-dashboard"
PLIST="$HOME/Library/LaunchAgents/$LABEL.plist"
UID_N="$(id -u)"

mkdir -p "$APP_DIR/logs"

managed() { launchctl print "gui/$UID_N/$LABEL" >/dev/null 2>&1; }
pid_on_port() { lsof -nP -iTCP:"$PORT" -sTCP:LISTEN -t 2>/dev/null | head -1; }
running() { [[ -n "$(pid_on_port)" ]]; }
url() { echo "http://127.0.0.1:$PORT/"; }

wait_up() {
  local i
  for i in 1 2 3 4 5 6; do
    running && return 0
    sleep 0.5
  done
  return 1
}

start() {
  if running; then
    echo "已在运行 (pid $(pid_on_port))  $(url)"
    return 0
  fi
  [[ -x "$BIN" ]] || { echo "找不到可执行文件: $BIN" >&2; return 1; }

  if [[ -f "$PLIST" ]]; then
    managed || launchctl bootstrap "gui/$UID_N" "$PLIST"
    launchctl kickstart "gui/$UID_N/$LABEL" >/dev/null 2>&1
  else
    (cd "$APP_DIR" && PORT="$PORT" nohup "$BIN" >> "$LOG" 2>&1 & echo $! > "$PIDFILE")
  fi

  if wait_up; then
    echo "已启动: $(url)"
  else
    echo "启动失败，日志见 $LOG" >&2
    return 1
  fi
}

stop() {
  if managed; then
    launchctl bootout "gui/$UID_N/$LABEL" >/dev/null 2>&1
    echo "已停止，并卸载了开机自启任务"
  elif running; then
    kill "$(pid_on_port)" 2>/dev/null
    rm -f "$PIDFILE"
    echo "已停止"
  else
    echo "未在运行"
  fi
}

autostart() {
  [[ -f "$PLIST" ]] || { echo "缺少 $PLIST" >&2; return 1; }
  managed || launchctl bootstrap "gui/$UID_N" "$PLIST"
  managed || launchctl enable "gui/$UID_N/$LABEL"
  start
  echo "开机自启已启用：登录后自动运行在 $(url)"
}

no_autostart() {
  if managed; then
    launchctl bootout "gui/$UID_N/$LABEL" >/dev/null 2>&1
  fi
  echo "开机自启已关闭（plist 保留在 $PLIST，可用 autostart 重新启用）"
}

status() {
  if running; then
    local src="手动启动"
    managed && src="launchd 开机自启"
    echo "运行中 (pid $(pid_on_port), $src)  $(url)"
    [[ -f "$PLIST" ]] && managed && echo "开机自启: 已启用" || echo "开机自启: 未启用"
  else
    echo "未运行"
    return 1
  fi
}

case "${1:-status}" in
  start)       start ;;
  stop)        stop ;;
  restart)     stop; sleep 1; start ;;
  status)      status ;;
  open)        start >/dev/null; open "$(url)" ;;
  autostart)   autostart ;;
  no-autostart) no_autostart ;;
  url)         url ;;
  *) echo "用法: $0 {start|stop|restart|status|open|autostart|no-autostart|url}" >&2; exit 1 ;;
esac
