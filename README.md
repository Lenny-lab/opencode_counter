# OpenCode 用量三件套 (opencode-usage-kit)

本地运行、只读统计、不上传任何数据的 OpenCode 用量监测套件，包含三个可独立安装的组件：

| 组件 | 说明 |
| --- | --- |
| **OpenCode Stats.app** | 菜单栏实时显示今日 token / 费用，Overview、Sessions、Models、Projects 四个统计页；支持按模型 token 排序、子会话（subagent）用量归并 |
| **opencode-token-dashboard** | 整机用量看板，本地 Web 服务 `http://127.0.0.1:8765/`，含 2 小时粒度活跃度热力图、每日趋势、模型/Provider 拆解 |
| **opencode-dynamic-context-pruning (DCP)** | opencode 全局插件，上下文接近上限时自动裁剪，缓解长会话爆上下文 |

统计口径：`input + output + reasoning + cache.read + cache.write`，数据源为
`~/.local/share/opencode/opencode.db`（也支持 `OPENCODE_DATA_DIR` / `XDG_DATA_HOME`）。

## 目录结构

```
bin/                  双架构（arm64 + x86_64）看板二进制 opencode-token-dashboard
dashboard.sh          看板启停脚本（start/stop/status/open/autostart）
installer/            DMG 内的三个安装脚本与使用说明（README.md）
release/              打包产物 opencode-usage-kit-1.0.dmg 与暂存目录（不入库）
vendor/
  opencode-stats-main/                菜单栏应用源码（Xcode / SwiftUI）
  opencode-token-dashboard-master/    看板源码（Rust axum + React/Vite）
  opencode-dynamic-context-pruning-master/  DCP 插件源码
```

## 安装

使用 Release 里的 `opencode-usage-kit-1.0.dmg`，双击依次运行三个 `.command` 脚本，
详见 `installer/README.md`。无 Apple Developer 公证时，脚本会自动去除
`com.apple.quarantine` 属性。

## 本地构建

**菜单栏应用**（输出 universal 二进制）：

```bash
cd vendor/opencode-stats-main
xcodebuild -project "OpenCode Stats.xcodeproj" -scheme "OpenCode Stats" \
  -configuration Release -derivedDataPath build \
  ARCHS="arm64 x86_64" ONLY_ACTIVE_ARCH=NO \
  CODE_SIGN_STYLE=Manual CODE_SIGN_IDENTITY=- DEVELOPMENT_TEAM= \
  CODE_SIGNING_REQUIRED=NO build
```

**看板**（前端 + 后端 + 合并双架构）：

```bash
cd vendor/opencode-token-dashboard-master
npm run build                       # 生成 static/
cargo build --release               # 本机架构
cargo build --release --target x86_64-apple-darwin
lipo -create target/release/opencode-token-dashboard \
              target/x86_64-apple-darwin/release/opencode-token-dashboard \
              -output ../../bin/opencode-token-dashboard
```

## 运行看板

```bash
./dashboard.sh start     # 后台启动
./dashboard.sh open      # 打开 http://127.0.0.1:8765
./dashboard.sh status    # 查看状态 / 端口
./dashboard.sh stop
```

开机自启由 LaunchAgent `com.putou.opencode-token-dashboard` 提供。

## License

- 本仓库自定义部分：仅供个人使用统计，不上传数据。
- `vendor/` 下为各自上游项目的源码，遵循其原始 LICENSE。
