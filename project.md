# 项目状态

## 当前进度

- 已完成 gpui-kit 0.6 依赖迁移。
- 根应用和 `crates/player_core` 统一使用 `gpui_kit` 导出的 GPUI、Component、Assets 和 Platform 类型。
- HTTP client 使用 workspace 中的 `gpui-pre-reqwest-client` 依赖。
- 保留 GStreamer、播放器、转码器、抓取器和 Windows 打包基础设施。
- 保留现有 fern 日志初始化；日志目录为 `logs/`，文件格式为 `logs/YYYY-MM-DD.log`。
- 未新增 GUI 测试，按项目约束以编译和手工启动冒烟验证 GUI。

## 验证结果

- `cargo fmt --all -- --check`：通过。
- `cargo check --workspace --locked`：通过。
- `cargo check -p player_core --locked`：通过。
- `cargo check -p build_windows --locked`：通过。
- `cargo tree -i gpui-pre`：统一为 `gpui-pre 0.3.4`。
- `cargo tree -i gpui-pre-platform`：统一为 `gpui-pre-platform 0.3.4`。
- 旧 Zed GPUI 和旧 `longbridge/gpui-component` Git 源检索：无结果。
- `cargo run --locked`：源码编译通过，但本机链接阶段缺少 GStreamer 导入库 `gstvideo-1.0.lib`，需要先修复本机 GStreamer 安装或路径后再执行窗口及媒体播放冒烟。

## 目录与文件职责

| 目录/文件 | 主要功能 |
| --- | --- |
| `Cargo.toml` | workspace、根应用依赖和 gpui-kit 统一依赖入口。 |
| `Cargo.lock` | Cargo 生成的依赖锁定结果；当前由仓库既有 `.gitignore` 忽略。 |
| `src/main.rs` | 应用入口、日志初始化、GStreamer 初始化、HTTP client、合并资源和主窗口启动。 |
| `src/com/request.rs` | 网络请求封装和文件下载。 |
| `src/drive/mod.rs` | 网络媒体数据结构和下载接口。 |
| `src/component/` | 窗口、标题栏、可调整面板、拖拽列表和颜色等通用 UI 组件。 |
| `src/gui/home/` | 首页布局、侧边栏和视频播放器 UI。 |
| `src/gui/music_page/` | 音乐页面、音乐播放器和搜索/播放交互 UI。 |
| `src/gui/video_page/` | 视频搜索、推荐、详情、列表和播放器页面 UI。 |
| `src/plugins/extractor/` | 音视频平台配置、搜索、推荐、抓取和播放资源解析。 |
| `src/plugins/extractor/video/play.rs` | 视频播放资源请求和 URL 解析。 |
| `crates/player_core/Cargo.toml` | 播放核心 crate 依赖声明，统一使用 gpui-kit。 |
| `crates/player_core/src/core/` | 播放器核心状态、任务、管线、媒体控制和运行时。 |
| `crates/player_core/src/external.rs` | 播放核心对外 API 和外部窗口/媒体接口。 |
| `crates/player_core/src/state.rs` | 播放器全局状态、事件和订阅类型。 |
| `crates/player_core/src/internal/control.rs` | 播放器内部窗口控制和拖拽控制。 |
| `crates/player_core/src/export/` | 媒体导出请求和导出流程。 |
| `crates/player_core/src/transcoder/` | GStreamer 转码器和 URI 解码流程。 |
| `crates/player_core/src/ui/` | 播放器标题栏、控制条、画面和 Markdown 错误展示。 |
| `crates/build_windows/` | Windows GStreamer runtime 准备、校验、构建和打包；本次未修改。 |
