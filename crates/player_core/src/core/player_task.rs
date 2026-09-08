use crate::{PlatState, PlayCore, PlayCoreMediaType};
use gpui_kit::*;
use gstreamer::prelude::*;
use std::time::Duration;

impl PlayCore {
    pub(crate) fn start_loading_timeout_task(&mut self, cx: &mut Context<Self>) {
        if self.task.loading_timeout_task.is_some() {
            return;
        }
        let source = self.player_static.url.clone();
        let session_id = self.pipeline.session_id;
        if source.starts_with("file://") {
            return;
        }

        let mut loading_progress = self.pipeline.loading_progress;
        self.task.loading_timeout_task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_secs(60))
                    .await;
                let next_progress = this
                    .update(cx, |this, cx| {
                        if !this.pipeline.is_current_session(session_id)
                            || this.player_static.url != source
                        {
                            return None;
                        }
                        if !matches!(this.pipeline.state, PlatState::Loading) {
                            this.task.loading_timeout_task = None;
                            return None;
                        }
                        if this.pipeline.loading_progress != loading_progress {
                            return Some(this.pipeline.loading_progress);
                        }

                        this.reset_pipeline();
                        this.pipeline.state = PlatState::Error("加载媒体源超时".to_string());
                        this.task.loading_timeout_task = None;
                        cx.notify();
                        None
                    })
                    .ok()
                    .flatten();
                let Some(next_progress) = next_progress else {
                    break;
                };
                loading_progress = next_progress;
            }
        }));
    }

    pub(crate) fn start_progress_task(&mut self, cx: &mut Context<Self>) {
        if self.task.progress_task.is_some() {
            return;
        }
        let session_id = self.pipeline.session_id;
        self.task.progress_task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(200))
                    .await;
                if !this
                    .update(cx, |this, cx| this.update_progress(session_id, cx))
                    .unwrap_or(false)
                {
                    break;
                }
            }
        }));
    }

    pub(crate) fn start_frame_task(&mut self, cx: &mut Context<Self>) {
        if self.player_static_info.media_type != PlayCoreMediaType::Video {
            return;
        }
        if self.task.frame_task.is_some() {
            return;
        }
        let session_id = self.pipeline.session_id;
        self.task.frame_task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(30))
                    .await;
                if !this
                    .update(cx, |this, cx| this.update_frame(session_id, cx))
                    .unwrap_or(false)
                {
                    break;
                }
            }
        }));
    }

    fn update_progress(&mut self, session_id: u64, cx: &mut Context<Self>) -> bool {
        if !self.pipeline.is_current_session(session_id) {
            return false;
        }
        if let Some(pipeline) = &self.pipeline.pipeline {
            if let Some(position) = pipeline.query_position::<gstreamer::ClockTime>() {
                self.progress.position = self.clock_to_duration(position);
            }
            if self
                .progress
                .duration
                .map(|duration| duration.is_zero())
                .unwrap_or(true)
            {
                if let Some(duration) = pipeline.query_duration::<gstreamer::ClockTime>() {
                    let duration = self.clock_to_duration(duration);
                    if !duration.is_zero() {
                        self.progress.duration = Some(duration);
                    }
                }
            }
        }

        let keep_running = matches!(self.pipeline.state, PlatState::Loading | PlatState::Playing)
            || self.progress.is_dragging;
        if !keep_running {
            self.task.progress_task = None;
        }
        cx.notify();
        keep_running
    }

    fn update_frame(&mut self, session_id: u64, cx: &mut Context<Self>) -> bool {
        if !self.pipeline.is_current_session(session_id) {
            return false;
        }
        if self.player_static_info.media_type != PlayCoreMediaType::Video {
            self.task.frame_task = None;
            return false;
        }
        if let Some(frame) = self.frame.images.submit_latest_frame() {
            self.mark_loading_progress();
            self.player_static_info.frame_info = frame;
            self.frame.refresh_pending = false;
            self.mark_video_present();
            if matches!(self.pipeline.state, PlatState::Loading) {
                self.pipeline.state = PlatState::Playing;
            }
            cx.notify();
        }

        let keep_running = self.pipeline.pipeline.is_some()
            && (matches!(self.pipeline.state, PlatState::Loading | PlatState::Playing)
                || (self.pipeline.state == PlatState::Paused && self.frame.refresh_pending));
        if !keep_running {
            self.task.frame_task = None;
        }
        keep_running
    }
}
