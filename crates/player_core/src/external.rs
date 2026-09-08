use crate::{PlatState, PlayCore, PlayStatic};
use anyhow::anyhow;
use gpui_kit::component::Root;
use gpui_kit::http_client::Url;
use gpui_kit::{
    App, AppContext, Bounds, Context, EntityId, ExternalPaths, IntoElement, Point, SharedString,
    TitlebarOptions, Window, WindowBounds, WindowDecorations, WindowId, WindowOptions, px, size,
};
use reqwest::header::HeaderMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use uuid::Uuid;

#[derive(Clone, Copy, Debug)]
pub struct PlayCoreProgress {
    pub position: Duration,
    pub duration: Option<Duration>,
    pub ratio: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlayCoreMediaType {
    #[default]
    Unknown,
    Audio,
    Video,
}

#[derive(Clone, Debug)]
pub struct PlayCoreViewState {
    pub player: PlayStatic,
    pub is_idle: bool,
    pub is_loading: bool,
    pub is_playing: bool,
    pub is_paused: bool,
    pub error_message: Option<String>,
    pub position: Duration,
    pub duration: Option<Duration>,
    pub volume: f32,
    pub frame_aspect: f32,
    pub frame_width: f32,
    pub frame_height: f32,
    pub frame_rate: f64,
    pub codec: Option<String>,
    pub media_type: PlayCoreMediaType,
}

impl PlayCore {
    pub fn _progress(&self) -> PlayCoreProgress {
        let duration = self.progress.duration;
        let position = self.display_position();

        PlayCoreProgress {
            position,
            duration,
            ratio: self.progress_ratio(),
        }
    }

    pub fn _view_state(&self) -> PlayCoreViewState {
        let (is_idle, is_loading, is_playing, is_paused, error_message) = match &self.pipeline.state
        {
            PlatState::UnLoading => (true, false, false, false, None),
            PlatState::Loading => (false, true, false, false, None),
            PlatState::Playing => (false, false, true, false, None),
            PlatState::Paused => (false, false, false, true, None),
            PlatState::Error(message) => (false, false, false, false, Some(message.clone())),
        };

        PlayCoreViewState {
            player: self.player_static.clone(),
            is_idle,
            is_loading,
            is_playing,
            is_paused,
            error_message,
            position: self.progress.position,
            duration: self.progress.duration,
            volume: self.volume.value,
            frame_aspect: self.player_static_info.frame_info.aspect_ratio(),
            frame_width: self.player_static_info.frame_info.width as f32,
            frame_height: self.player_static_info.frame_info.height as f32,
            frame_rate: self.player_static_info.frame_info.frame_rate,
            codec: self.player_static_info.codec.clone(),
            media_type: self.player_static_info.media_type,
        }
    }

    pub fn _frame_ui(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.frame.images.drain_retired_images(window);
        self.render_frame(cx)
    }

    pub fn _progress_ui(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.player_progress_control_ui(window, cx)
    }

    pub fn _volume_ui(&self, cx: &mut Context<Self>) -> impl IntoElement {
        self.player_volume_control_ui(cx)
    }

    pub fn _duration_ui(&self) -> impl IntoElement {
        let total = self.progress.duration.unwrap_or(Duration::ZERO);
        let position = self.display_position();
        self.player_duration_display_ui(position, total)
    }

    pub fn _file_drop_source(&self, paths: &ExternalPaths) -> Option<PlayStatic> {
        let Some(path) = paths.paths().iter().find(|path| path.is_file()) else {
            return None;
        };
        let Some(url) = Url::from_file_path(Path::new(path))
            .ok()
            .map(|url| url.to_string())
        else {
            return None;
        };

        Some(PlayStatic {
            id: Uuid::new_v4().to_string(),
            title: Path::new(path)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "本地媒体".to_string()),
            url,
            headers: HeaderMap::new(),
        })
    }

    pub fn _open_window(
        window: &mut Window,
        cx: &mut App,
        title: &str,
    ) -> anyhow::Result<(WindowId, EntityId)> {
        let player_entity_id = Arc::new(Mutex::new(None));
        let player_entity_id_for_window = player_entity_id.clone();
        let options = window_center_settings(window, 1400., 800., title);
        let handler = cx
            .open_window(options, move |window, app| {
                window.on_window_should_close(app, |window, _| {
                    window.remove_window();
                    false
                });

                let view = app.new(|cx| PlayCore::new(window, cx));
                if let Ok(mut player_entity_id) = player_entity_id_for_window.lock() {
                    *player_entity_id = Some(view.entity_id());
                }
                app.new(|cx| Root::new(view, window, cx))
            })
            .map_err(|error| anyhow!("创建播放器窗口失败: {error}"))?;
        let player_entity_id = player_entity_id
            .lock()
            .map_err(|_| anyhow!("播放器窗口实体状态已损坏"))?
            .ok_or_else(|| anyhow!("播放器窗口实体未创建"))?;
        Ok((handler.window_id(), player_entity_id))
    }
}

fn window_center_settings(window: &mut Window, w: f32, h: f32, title: &str) -> WindowOptions {
    let parent_bounds = window.bounds();
    let window_size = size(px(w), px(h));
    let bounds = Bounds {
        origin: Point {
            x: parent_bounds.origin.x + (parent_bounds.size.width - px(w)) / 2.0,
            y: parent_bounds.origin.y + (parent_bounds.size.height - px(h)) / 2.0,
        },
        size: window_size,
    };
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        window_min_size: Some(window_size),
        is_resizable: true,
        titlebar: Some(TitlebarOptions {
            title: Some(SharedString::from(title.to_string())),
            appears_transparent: true,
            ..Default::default()
        }),
        window_decorations: Some(WindowDecorations::Client),
        ..Default::default()
    }
}
