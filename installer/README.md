# OpenCode 用量三件套 (opencode-usage-kit)

本 DMG 包含三个可独立安装的组件，全部**本地运行、只读统计、不上传任何数据**：

| 组件 | 作用 |
| --- | --- |
| OpenCode Stats.app | 菜单栏实时显示今日 token、总用量/费用，Overview / Sessions / Models / Projects 四个统计页（已支持按模型 token 排序、子会话统计） |
| opencode-token-dashboard | 整机用量看板，本地 Web 服务 `http://127.0.0.1:8765/`，登录后自动启动 |
| opencode-dynamic-context-pruning | opencode 全局插件，上下文接近上限时自动裁剪，缓解长会话爆上下文 |

## 系统要求

- macOS 13 (Ventura) 及以上，Apple Silicon 与 Intel 通用（二进制为 universal）
- 已安装 [opencode](https://opencode.ai)，并且本机存在数据库
  `~/.local/share/opencode/opencode.db`（或 `XDG_DATA_HOME/opencode/opencode.db`）

## 安装步骤

双击运行 DMG 内的脚本，按顺序执行（每步末尾按回车关闭窗口即可）：

1. **`1-安装菜单栏应用.command`**
   拷贝 `OpenCode Stats.app` 到 `/Applications`，自动去除隔离属性并启动。
   图标出现在屏幕右上角菜单栏：左侧点击看统计面板，右侧点击看 Total/Today 汇总菜单。
2. **`2-安装用量看板.command`**
   安装到 `~/.local/share/opencode-token-dashboard/`，写入 LaunchAgent 开机自启，
   并提供控制命令 `opencode-dashboard {start|stop|restart|open|status|uninstall}`（位于 `~/.local/bin`）。
3. **`3-安装DCP上下文插件.command`**
   自动执行 `opencode plugin -g @tarquinen/opencode-dcp@latest`，重启 opencode 生效。

也可以手动安装：把 `OpenCode Stats.app` 拖到 `Applications`，其余组件按脚本内命令操作。

## 首次打开提示“无法验证开发者 / 已损坏”？

应用为本地 ad-hoc 签名（未购买 Apple 开发者证书），属正常现象，任选其一：

- 右键点按应用 → **打开** → 再次点“打开”；
- 或运行 DMG 内的 `1-安装菜单栏应用.command`（脚本会自动 `xattr -dr com.apple.quarantine`）；
- 或终端执行：`xattr -dr com.apple.quarantine "/Applications/OpenCode Stats.app"`。

## 常见问题

- **菜单栏显示“未找到数据库”**：本机还没跑过 opencode，或数据目录非默认路径。
  可用 `OPENCODE_DATA_DIR=/your/data/dir` 启动应用指定目录。
- **看板打不开**：`opencode-dashboard status` 查看，`opencode-dashboard start` 启动，
  日志在 `/tmp/opencode-dashboard.log`；端口被占用时用 `PORT=8766 opencode-dashboard open`。
- **模型用量没变化**：统计已包含 subagent 子会话；若仍异常，确认数据库路径与 opencode 实际使用的一致。

## 卸载

```zsh
opencode-dashboard uninstall                 # 看板 + 开机自启 + 控制命令
rm -rf "/Applications/OpenCode Stats.app"    # 菜单栏应用
opencode plugin -g @tarquinen/opencode-dcp@latest --uninstall 2>/dev/null || true
```

## 数据口径

- 仅读取本机 opencode SQLite 数据库，不联网、不上传。
- token = input + output + reasoning + cache.read + cache.write。
- 模型维度按模型名跨 provider 合并，并统计子会话（subagent）用量。

---
仓库：<https://github.com/Lenny-lab/opencode_counter>
