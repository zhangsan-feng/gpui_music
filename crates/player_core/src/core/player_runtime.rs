use gpui_kit::*;
use gstreamer as gst;
use image::{Frame, RgbaImage};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::PlayCoreMediaType;

pub struct ProgressDrag;

#[derive(Clone, Copy)]
pub struct VolumeDrag;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PlatState {
    UnLoading,
    Loading,
    Playing,
    Paused,
    Error(String),
}

#[derive(Default)]
pub(crate) struct PlayerStaticInfo {
    pub(crate) codec: Option<String>,
    pub(crate) media_type: PlayCoreMediaType,
    pub(crate) frame_info: FrameInfo,
}

#[derive(Default)]
pub(crate) struct ProgressState {
    pub(crate) duration: Option<Duration>,
    pub(crate) position: Duration,
    pub(crate) is_dragging: bool,
    pub(crate) pending_seek_position: Option<Duration>,
    pub(crate) bar_bounds: Option<Bounds<Pixels>>,
}

pub(crate) struct VolumeState {
    pub(crate) value: f32,
    pub(crate) bar_bounds: Option<Bounds<Pixels>>,
}

impl Default for VolumeState {
    fn default() -> Self {
        Self {
            value: 0.6,
            bar_bounds: None,
        }
    }
}

pub(crate) struct FrameState {
    pub(crate) images: FramePipeline,
    pub(crate) refresh_pending: bool,
    pub(crate) surface_bounds: Option<Bounds<Pixels>>,
}

impl Default for FrameState {
    fn default() -> Self {
        Self {
            images: FramePipeline::default(),
            refresh_pending: false,
            surface_bounds: None,
        }
    }
}

pub(crate) struct PipelineRuntime {
    pub(crate) session_id: u64,
    pub(crate) state: PlatState,
    pub(crate) pipeline: Option<gst::Element>,
    pub(crate) segment_end: Option<Duration>,
    pub(crate) loading_progress: u64,
    pub(crate) last_buffering_percent: Option<i32>,
    pub(crate) buffering_paused: bool,
}

impl Default for PipelineRuntime {
    fn default() -> Self {
        Self {
            session_id: 0,
            state: PlatState::UnLoading,
            pipeline: None,
            segment_end: None,
            loading_progress: 0,
            last_buffering_percent: None,
            buffering_paused: false,
        }
    }
}

impl PipelineRuntime {
    pub(crate) fn invalidate_session(&mut self) -> u64 {
        self.session_id = self.session_id.wrapping_add(1);
        self.session_id
    }

    pub(crate) fn is_current_session(&self, session_id: u64) -> bool {
        self.session_id == session_id
    }
}

#[derive(Default)]
pub(crate) struct TaskRuntime {
    pub(crate) progress_task: Option<Task<()>>,
    pub(crate) frame_task: Option<Task<()>>,
    pub(crate) bus_watch_task: Option<Task<()>>,
    pub(crate) loading_timeout_task: Option<Task<()>>,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct FrameInfo {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) frame_rate: f64,
}

impl FrameInfo {
    pub(crate) fn aspect_ratio(self) -> f32 {
        if self.width == 0 || self.height == 0 {
            return 16.0 / 9.0;
        }
        (self.width as f32 / self.height as f32).max(0.01)
    }
}

#[derive(Default)]
pub(crate) struct FrameBuffer {
    pub(crate) info: FrameInfo,
    pub(crate) data: Vec<u8>,
    pub(crate) seq: u64,
}

pub(crate) struct FramePipeline {
    latest_frame: Arc<Mutex<FrameBuffer>>,
    last_presented_sequence: u64,
    current_image: Option<Arc<RenderImage>>,
    retired_images: Vec<Arc<RenderImage>>,
}

impl Default for FramePipeline {
    fn default() -> Self {
        Self {
            latest_frame: Arc::new(Mutex::new(FrameBuffer::default())),
            last_presented_sequence: 0,
            current_image: None,
            retired_images: Vec::new(),
        }
    }
}

impl FramePipeline {
    pub(crate) fn latest_frame(&self) -> Arc<Mutex<FrameBuffer>> {
        self.latest_frame.clone()
    }

    pub fn current_image(&self) -> Option<Arc<RenderImage>> {
        self.current_image.clone()
    }

    pub(crate) fn reset(&mut self) {
        if let Some(image) = self.current_image.take() {
            self.retired_images.push(image);
        }
        self.latest_frame = Arc::new(Mutex::new(FrameBuffer::default()));
        self.last_presented_sequence = 0;
    }

    pub(crate) fn submit_latest_frame(&mut self) -> Option<FrameInfo> {
        if !self.retired_images.is_empty() {
            return None;
        }

        let (seq, info, data) = {
            let mut frame = self.latest_frame.lock().unwrap();
            if frame.seq == self.last_presented_sequence
                || frame.info.width == 0
                || frame.info.height == 0
            {
                return None;
            }
            (frame.seq, frame.info, std::mem::take(&mut frame.data))
        };

        let image = RgbaImage::from_raw(info.width, info.height, data)?;
        let image = Arc::new(RenderImage::new(vec![Frame::new(image)]));
        if let Some(old) = self.current_image.replace(image) {
            self.retired_images.push(old);
        }
        self.last_presented_sequence = seq;

        Some(info)
    }

    pub(crate) fn drain_retired_images(&mut self, window: &mut Window) {
        for image in self.retired_images.drain(..) {
            let _ = window.drop_image(image);
        }
    }

    pub(crate) fn release_images(&mut self, cx: &mut App) {
        if let Some(image) = self.current_image.take() {
            cx.drop_image(image, None);
        }
        for image in self.retired_images.drain(..) {
            cx.drop_image(image, None);
        }
    }
}
