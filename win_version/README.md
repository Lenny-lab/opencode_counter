# OpenCode Stats — Windows 版

`vendor/opencode-stats-main/` 里那套 macOS 菜单栏应用的 Windows 改写版。托盘常驻 + 独立看板窗口，
数据全部来自 OpenCode 自己写在磁盘上的 `opencode.db`，只读打开，不联网，不写回。

原项目一行没动，这个目录是从零写的。

## 界面

| 入口 | 说明 |
|---|---|
| 托盘图标 | 显示**今日 Tokens**（`1.2M` 这种紧凑写法，图标里用 3×5 点阵字渲染，缩到 16px 仍可读）。悬停提示里是今日 Tokens + 今日成本 |
| 托盘左键 | 弹出 360×520 面板：总览 / 会话 / 模型 / 项目四个标签页 |
| 托盘右键 | 菜单：打开看板、刷新、退出 |
| 看板窗口 | 1180×800（最小 1000×660）：热力图、每日趋势、服务商环形图、模型/项目/工具榜、缓存分析 |

数据库一变（OpenCode 写完消息落盘）两个窗口都会自动刷新，不用手动点。

## 统计口径

和 macOS 版保持一致，数字可以直接对照：

- **总 Tokens** = `输入 + 输出 + 推理 + 缓存读 + 缓存写`。缓存读是计费的，所以算进去。
- **成本**只做累加，不重算。`message.data.cost` 是 OpenCode 自己按 models.dev 定价算好的
  （推理 token 按输出价计费，除以 1e6）。代码里**没有任何价格表**——再造一份只会和上游对不上。
- **子会话**（`session.parent_id` 非空）并入父会话，只并一层。子会话的 token 算，运行时长不算，
  避免重复计时。
- **同一个模型**按模型名合并，多个服务商收进一个列表：`MiniMaxAI/MiniMax-M3` 会显示成
  `gmi, gmi-minimax`。
- **按会话过滤的统计**（总量、模型、服务商、项目、每会话 token、工具调用）用 `session.time_created`；
  **按天画的东西**（趋势、热力图、今日、缓存分析）用 `message.time_created`。
  混用会让一个跑了三天的会话在它没被使用的那几天也计入。
- **天跨度**是首末会话之间的自然日数量，不是有活动的天数。
- **每会话平均/中位 token**不含推理 token——这是历史惯例，衡量的是 prompt 侧的开销。
- 缓存命中率 = `缓存读 / (输入 + 缓存读 + 缓存写)`，分母为 0 时记 0。

### 缓存缺口怎么算的

相邻两条消息（同一会话内）配对：

- 换会话、换模型或换服务商 → 断开
- 前一条 `cache_read == 0`（冷启动，没东西可缓存）→ 断开
- 两条之间发生过压缩 → 断开
- 后一条 `total == 0`（中止或报错）→ 跳过，不配对

然后 `期望 = 前一条 total`，`缺口 = max(0, 前一条 total - 后一条 cache_read)`，**记在后一条头上**。
实现是流式的，只留一条 pending 消息，内存 O(1)。

### 已知的一处有意的偏差

最近会话列表里的成本，macOS 版只算会话自己的消息，不含子会话。这是那个应用里唯一一处不对称的地方，
这里改成**含子会话**——让会话列表和其余所有统计口径一致。

## 数据库在哪

按顺序找，取第一个存在的：

1. `OPENCODE_DB_PATH`（完整文件路径）
2. `OPENCODE_DATA_DIR\opencode.db`
3. `XDG_DATA_HOME\opencode\opencode.db`
4. `%LOCALAPPDATA%\opencode\`
5. `%APPDATA%\opencode\`
6. `%USERPROFILE%\.local\share\opencode\`
7. `%USERPROFILE%\.opencode\`

第 4～7 项里的目录会扫一遍：文件名以 `opencode` 开头、不以 `opencode-auth` 结尾、后缀 `.db`、
非空文件，按 `max(db, db-wal)` 的修改时间倒序。第一个是主库，其余在看板底部列出来但不参与统计。

> OpenCode 在所有平台上都用 `~/.local/share/opencode`，Windows 上 `~` 就是 `%USERPROFILE%`。
> 所以第 6 项才是绝大多数机器上真正生效的那个。

数据库打不开时不是抛异常，而是渲染一个「找不到数据库」的页面，告诉你查了哪些路径。

## 数据库结构

支持两代布局，**两张表都在时选行数多的那张**（迁移会留下旧表）：

- **V1**：`message.data` 里是 JSON，角色在 `$.role`，工具调用在 `part` 表
- **V2**：`session_message` 有独立的 `type` 列，parts 内联在 `data.content`

`message` 表没有 `data` 列 → 当作 V1 不存在处理。

## 怎么构建

需要 [Rust](https://rustup.rs/)（1.77+）、[Node.js](https://nodejs.org/)（20.19+ 或 22.12+）、
以及 Windows 上的 [WebView2 运行时](https://developer.microsoft.com/microsoft-edge/webview2/)
（Win11 自带，Win10 可能要单独装）。Tauri 的 Windows 工具链要
[WebView2 Build](https://learn.microsoft.com/microsoft-edge/webview2/) 和
[Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)（勾选
"Desktop development with C++"）。

```powershell
cd win_version
npm install
npm run tauri:build
```

产物在 `src-tauri/target/release/bundle/`：

- `nsis/OpenCode Stats_<版本>_x64-setup.exe` — 装到用户目录，推荐
- `msi/OpenCode Stats_<版本>_x64_en-US.msi` — 走组策略分发时用

要便携版就把 `src-tauri/target/release/opencode-stats-win.exe` 连同
`WebView2Loader.dll` 一起拷走。

开发模式：

```powershell
npm run tauri:dev
```

单独调前端（不启动 Tauri，命令会失败）：

```powershell
npm run dev
```

## 测试

```powershell
cd src-tauri
cargo test
```

- `src/db.rs`、`src/cache.rs`、`src/stats.rs` 里的单元测试锁住各个口径
- `tests/aggregation.rs` 在一个手工搭的 V1 库上跑完整快照，验证用户轮次不计入、子会话并入父会话、
  模型跨服务商合并、缓存缺口归属、`total: -1` 被重算
- `examples/live_probe.rs` 跑真实数据库，用来在 OpenCode 改结构时第一时间发现问题：

```powershell
cargo run --release --example live_probe
cargo run --release --example live_probe -- 90
```

## 和 macOS 版的差异

除了上面写明的偏差，还有这些：

| | macOS 版 | 这里 |
|---|---|---|
| 存储格式 | SwiftUI + AppKit | Tauri 2（Rust + React） |
| 托盘/菜单栏 | 菜单栏文字 `1.2M · $0.42` | Windows 通知区只有 16px，图里塞不下文字。数字用点阵字画进图标，完整数字放悬停提示 |
| 刷新 | `DispatchSource` 文件描述符监听 | 5 秒轮询 `db` + `db-wal` 的修改时间。省掉一个跨平台依赖，对一个悬停才看的面板来说这个精度够了 |
| 设置 | 偏好终端（`preferredTerminal`） | 没有。那是 macOS 专属的 |
| CSV 导出 | 有 | 没有 |
| 字体 | Geist + JetBrains Mono（内置） | Segoe UI Variable + Cascadia Mono（系统自带）。Windows 上原生字体比塞一个网络字体更合适 |
| 颜色 | 浅色/深色 | 只做深色 |

## 目录

```
src/                     前端
  App.tsx                按窗口标签决定渲染面板还是看板
  components/            Popover / Dashboard / Heatmap / TrendChart / 各榜单
  lib/                   ipc.ts（invoke 封装）、format.ts（数字与相对时间）、useSnapshot.ts
  types.ts               和 Rust serde 输出对应的类型（camelCase）
src-tauri/
  src/db.rs              数据库发现、只读连接、V1/V2 结构探测
  src/stats.rs           全部聚合查询
  src/cache.rs           缓存缺口配对
  src/tray_icon.rs       托盘点阵图标的渲染
  src/lib.rs             Tauri 命令、窗口、托盘、监听线程
  capabilities/          权限（只允许调用自己的命令）
  icons/                 从 macOS 版 logo 生成
```
