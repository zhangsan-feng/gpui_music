use crate::state::{PlayCoreGlobalState, PlayCoreStateEvent};
use gpui::{
    Context, ExternalPaths, InteractiveElement, IntoElement, ParentElement, Render, Rgba, Styled,
    Window, WindowId, rgb,
};
use gpui_component::v_flex;
use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, HeaderMap, HeaderValue, USER_AGENT};

mod player_control;
mod player_event;
mod player_media;
mod player_pipeline;
mod player_runtime;
mod player_task;

pub use crate::download::download::{PlayCoreDownload, PlayCoreDownloadRequest};
pub use crate::export::export::{PlayCoreExport, PlayCoreExportRequest, PlayCoreExportTrim};
pub use crate::transcoder::transcoder::{
    PlayCoreRealtimeTranscodeRequest, PlayCoreTranscodeFormat, PlayCoreTranscodeRequest,
    PlayCoreTranscodeSession, PlayCoreTranscoder,
};
pub(crate) use player_runtime::{
    FrameState, PipelineRuntime, PlayerStaticInfo, ProgressState, TaskRuntime, VolumeState,
};
pub(crate) use player_runtime::{PlatState, ProgressDrag, VolumeDrag};

pub(crate) fn rgb_to_u32(r: u8, g: u8, b: u8) -> Rgba {
    rgb((r as u32) << 16 | (g as u32) << 8 | b as u32)
}

#[derive(Clone, Debug)]
pub struct PlayStatic {
    pub id: String,
    pub title: String,
    pub url: String,
    pub headers: HeaderMap,
}

impl Default for PlayStatic {
    fn default() -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/131.0.0.0 Safari/537.36",
            ),
        );
        headers.insert(ACCEPT, HeaderValue::from_static("*/*"));
        headers.insert(
            ACCEPT_LANGUAGE,
            HeaderValue::from_static("zh-CN,zh;q=0.9,en;q=0.8"),
        );

        Self {
            id: String::new(),
            title: String::new(),
            url: String::new(),
            headers,
        }
    }
}

pub struct PlayCore {
    pub(crate) player_static: PlayStatic,
    pub(crate) player_static_info: PlayerStaticInfo,
    pub(crate) progress: ProgressState,
    pub(crate) volume: VolumeState,
    pub(crate) frame: FrameState,
    pub(crate) pipeline: PipelineRuntime,
    pub(crate) task: TaskRuntime,
    pub(crate) window_id: WindowId,
}

impl PlayCore {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let window_id = window.window_handle().window_id();
        let mut player = Self {
            player_static: PlayStatic::default(),
            player_static_info: PlayerStaticInfo::default(),
            progress: ProgressState::default(),
            volume: VolumeState::default(),
            frame: FrameState::default(),
            pipeline: PipelineRuntime::default(),
            task: TaskRuntime::default(),
            window_id,
        };
        cx.on_release(|this, cx| {
            this.frame.images.release_images(cx);
        })
        .detach();
        player.init_subscribe(window_id, cx);
        player
    }

    fn init_subscribe(&mut self, window_id: WindowId, cx: &mut Context<Self>) {
        let state_handler = cx.global::<PlayCoreGlobalState>().0.clone();
        let self_entity_id = cx.entity_id().clone();
        cx.subscribe(
            &state_handler,
            move |this: &mut Self, _model, event: &PlayCoreStateEvent, cx| match event {
                PlayCoreStateEvent::TogglePlay(event_window_id, event_entity_id, data) => {
                    if event_window_id.as_u64() == window_id.as_u64()
                        && self_entity_id == *event_entity_id
                    {
                        this.player_static = data.clone();
                        this.retry(cx);
                        cx.notify();
                    }
                }
                PlayCoreStateEvent::PlayBackFished(..) => {}
            },
        )
        .detach();
    }
}

impl Render for PlayCore {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.frame.images.drain_retired_images(window);

        if !self.player_static.title.trim().is_empty() {
            window.set_window_title(&self.player_static.title);
        }

        v_flex()
            .size_full()
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _window, cx| {
                this._handle_file_drop(paths, cx);
            }))
            .bg(rgb_to_u32(255, 255, 255))
            .child(self.render_title_bar(window, cx))
            .child(
                v_flex()
                    .flex_grow_1()
                    .min_w_0()
                    .min_h_0()
                    .relative()
                    .child(self.render_frame(cx))
                    .child(self.render_control(window, cx)),
            )
            .into_any_element()
    }
}
