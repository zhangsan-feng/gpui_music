#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod com;
mod component;
mod drive;
mod gui;
mod plugins;

use gpui_kit::component::*;
use gpui_kit::*;
use log::{Level, info};
use player_core::{PlayCoreGlobalState, PlayCoreState};
use reqwest_client::ReqwestClient;
use rust_embed::RustEmbed;
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub fn logger_init(log_dir: impl AsRef<Path>, date_format: &str) {
    let log_dir = log_dir.as_ref();
    std::fs::create_dir_all(log_dir).expect("create log directory failed");

    let log_file = log_dir.join(format!("{}.log", chrono::Local::now().format(date_format)));

    fern::Dispatch::new()
        .format(|out, message, record| {
            let file = record.file().unwrap_or("<unknown>");
            let line = record.line().unwrap_or(0);
            out.finish(format_args!(
                "[{}] [{}] [{}] [{}:{}] {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.target(),
                file,
                line,
                message
            ))
        })
        // .filter(|metadata| {
        //     metadata.level() == Level::Info && !metadata.target().starts_with("symphonia")
        // })
        .level(log::LevelFilter::Info)
        // .level_for("gstreamer", log::LevelFilter::Debug)
        // .level(log::LevelFilter::Debug)
        // .level(log::LevelFilter::Trace)
        .chain(std::io::stdout())
        .chain(fern::log_file(&log_file).expect("open log file failed"))
        .apply()
        .expect("init logger failed");

    info!("init logger success: {}", log_file.display());
}

#[derive(RustEmbed)]
#[folder = "./src/icon"]
struct AssetFiles;

struct MergedAssets {
    local_directories: Vec<PathBuf>,
    component_assets: gpui_kit::assets::Assets,
}

impl AssetSource for MergedAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        for dir in &self.local_directories {
            let full_path = dir.join(path);
            if full_path.exists() {
                let bytes = std::fs::read(full_path)?;
                return Ok(Some(Cow::Owned(bytes)));
            }
        }

        let clean_path = path.trim_start_matches("icon/").trim_start_matches("/");
        if let Some(file) = AssetFiles::get(clean_path) {
            return Ok(Some(file.data));
        }

        self.component_assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut all_files = std::collections::HashSet::new();

        for dir in &self.local_directories {
            let full_path = dir.join(path);
            if full_path.is_dir() {
                if let Ok(entries) = std::fs::read_dir(full_path) {
                    for entry in entries.flatten() {
                        if let Some(name) = entry.file_name().to_str() {
                            all_files.insert(name.to_string());
                        }
                    }
                }
            }
        }

        let clean_path = path.trim_start_matches("icon/").trim_start_matches("/");
        for file_path in AssetFiles::iter() {
            if file_path.starts_with(clean_path) {
                all_files.insert(file_path.to_string());
            }
        }

        for file_path in self.component_assets.list(path)? {
            all_files.insert(file_path.to_string());
        }

        Ok(all_files.into_iter().map(SharedString::from).collect())
    }
}

#[tokio::main]
async fn main() {
    let runtime_mode = match build_windows::runtime::prepare_current_process() {
        Ok(mode) => mode,
        Err(error) => {
            eprintln!("GStreamer runtime validation failed: {error:#}");
            return;
        }
    };
    if let Err(error) = gstreamer::init() {
        eprintln!("GStreamer initialization failed: {error}");
        return;
    }
    logger_init("./logs", "%Y-%m-%d");
    info!(
        "GStreamer {} initialized in {:?} mode",
        gstreamer::version_string(),
        runtime_mode
    );

    let http_client = ReqwestClient::user_agent("gpui").unwrap();
    let assets = MergedAssets {
        local_directories: vec![PathBuf::from("/"), PathBuf::from("./src/icon")],
        component_assets: gpui_kit::assets::Assets,
    };

    gpui_kit::application()
        .with_http_client(Arc::new(http_client))
        .with_assets(assets)
        .run(move |cx| {
            let mut window_options = WindowOptions::default();
            let window_size = size(px(1400.), px(800.));
            window_options.window_bounds = Some(WindowBounds::centered(window_size, cx));
            window_options.window_min_size = Some(window_size);
            window_options.titlebar = Some(TitlebarOptions {
                title: None,
                // Hide the extractor titlebar; HomeView renders the compatible custom one.
                appears_transparent: true,
                traffic_light_position: None,
            });
            // Client decorations keep native resize/maximize hit testing available while
            // allowing the titlebar content to be drawn by GPUI.
            window_options.window_decorations = Some(WindowDecorations::Client);

            cx.open_window(window_options, |window, app| {
                window.on_window_should_close(app, |window, _| {
                    window.remove_window();
                    false
                });

                gpui_kit::init(app);

                app.new(|cx| {
                    let play_core_state = cx.new(|cx| PlayCoreState::new(cx));
                    cx.set_global(PlayCoreGlobalState::new(play_core_state));
                    let main_window = cx.new(|cx| gui::home::HomeView::new(window, cx));
                    Root::new(main_window, window, cx)
                })
            })
            .expect("Failed to create app");
        });
}
