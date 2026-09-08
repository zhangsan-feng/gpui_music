use crate::{PlatState, PlayCore, PlayCoreMediaType};
use gpui_kit::{Bounds, Context, Pixels, Point};
use gstreamer as gst;
use std::time::Duration;

impl PlayCore {
    pub(crate) fn playback_controls_enabled(&self) -> bool {
        !matches!(self.pipeline.state, PlatState::Loading)
    }

    pub(crate) fn toggle_play(&mut self, cx: &mut Context<Self>) {
        if !self.playback_controls_enabled() {
            return;
        }

        match &self.pipeline.state {
            PlatState::Playing => self.pause(cx),
            PlatState::Paused => self.resume_or_play(cx),
            PlatState::UnLoading | PlatState::Error(_) => self.play(cx),
            PlatState::Loading => {}
        }
    }

    pub(crate) fn retry(&mut self, cx: &mut Context<Self>) {
        if self.player_static.url.trim().is_empty() {
            self.reset_pipeline();
            cx.notify();
            return;
        }
        self.reset_pipeline();
        self.play(cx);
    }

    pub(crate) fn play(&mut self, cx: &mut Context<Self>) {
        if self.player_static.url.is_empty() || !self.playback_controls_enabled() {
            return;
        }

        if let Err(error) = self.ensure_pipeline(cx) {
            self.reset_pipeline();
            self.pipeline.state = PlatState::Error("播放失败".to_string());
            log::debug!("{}", error.backtrace());
            cx.notify();
            return;
        }

        if !self.set_playing() {
            self.reset_pipeline();
            self.pipeline.state = PlatState::Error("播放失败".to_string());
            log::warn!("[gst:play] pipeline failed to enter playing state");
            cx.notify();
            return;
        }

        self.start_progress_task(cx);
        cx.notify();
    }

    pub(crate) fn pause(&mut self, cx: &mut Context<Self>) {
        self.pause_pipeline();
        cx.notify();
    }

    pub(crate) fn resume(&mut self) -> bool {
        if !matches!(self.pipeline.state, PlatState::Paused) {
            return false;
        }
        if !self.set_playing() {
            return false;
        }
        self.pipeline.buffering_paused = false;
        self.pipeline.state = PlatState::Playing;
        true
    }

    pub(crate) fn pause_pipeline(&mut self) {
        if !self.playback_controls_enabled() {
            return;
        }
        if !self.set_paused() {
            return;
        }
        self.task.frame_task = None;
        self.frame.refresh_pending = false;
        self.pipeline.buffering_paused = false;
        self.pipeline.state = PlatState::Paused;
    }

    pub(crate) fn seek(&mut self, position: Duration, cx: &mut Context<Self>) -> bool {
        self.pipeline.segment_end = None;
        if self.pipeline.pipeline.is_some() {
            if !self.seek_pipeline(position) {
                return false;
            }
            self.progress.position = position;
            if self.pipeline.state == PlatState::Paused
                && self.player_static_info.media_type == PlayCoreMediaType::Video
            {
                self.frame.refresh_pending = true;
                self.start_frame_task(cx);
            }
            return true;
        }
        false
    }

    pub(crate) fn play_segment(&mut self, start: Duration, end: Duration) -> bool {
        if start >= end || !self.seek_segment_pipeline(start, end) || !self.set_playing() {
            return false;
        }
        self.pipeline.segment_end = Some(end);
        self.progress.position = start;
        self.frame.refresh_pending = false;
        self.pipeline.buffering_paused = false;
        self.pipeline.state = PlatState::Playing;
        true
    }

    pub(crate) fn set_volume(&mut self, volume: f32) -> f32 {
        self.volume.value = volume.clamp(0.0, 1.0);
        self.set_pipeline_volume(self.volume.value);
        self.volume.value
    }

    pub(crate) fn reset_pipeline(&mut self) {
        self.stop_pipeline_runtime();
        self.pipeline.state = PlatState::UnLoading;
        self.progress.duration = None;
        self.progress.position = Duration::ZERO;
        self.player_static_info.frame_info = Default::default();
        self.player_static_info.codec = None;
        self.player_static_info.media_type = PlayCoreMediaType::Unknown;
        self.frame.images.reset();
        self.clear_progress_interaction();
    }

    pub(crate) fn finish_pipeline(&mut self) {
        self.stop_pipeline_runtime();
        self.pipeline.state = PlatState::Paused;
        if let Some(duration) = self.progress.duration {
            self.progress.position = duration;
        }
        self.clear_progress_interaction();
    }

    fn stop_pipeline_runtime(&mut self) {
        self.pipeline.invalidate_session();
        self.reset_pipeline_state();
        self.task.progress_task = None;
        self.task.frame_task = None;
        self.task.bus_watch_task = None;
        self.task.loading_timeout_task = None;
        self.pipeline.last_buffering_percent = None;
        self.pipeline.buffering_paused = false;
    }

    pub(crate) fn clock_to_duration(&self, clock: gst::ClockTime) -> Duration {
        Duration::from_nanos(clock.nseconds())
    }

    pub(crate) fn duration_to_clock_time(duration: Duration) -> gst::ClockTime {
        gst::ClockTime::from_nseconds(duration.as_nanos().min(u64::MAX as u128) as u64)
    }

    pub(crate) fn display_position(&self) -> Duration {
        self.progress
            .pending_seek_position
            .filter(|_| self.progress.is_dragging)
            .unwrap_or(self.progress.position)
    }

    pub(crate) fn progress_ratio(&self) -> f32 {
        let Some(duration) = self
            .progress
            .duration
            .filter(|duration| !duration.is_zero())
        else {
            return 0.0;
        };
        (self.display_position().as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0)
    }

    pub(crate) fn bar_ratio(&self, position: Point<Pixels>, bounds: Bounds<Pixels>) -> f32 {
        ((position.x.as_f32() - bounds.origin.x.as_f32()) / bounds.size.width.as_f32().max(1.0))
            .clamp(0.0, 1.0)
    }

    pub(crate) fn get_progress_position(
        &self,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
    ) -> Option<Duration> {
        let total = self.progress.duration?;
        if total.is_zero() {
            return None;
        }
        Some(Duration::from_secs_f32(
            total.as_secs_f32() * self.bar_ratio(position, bounds),
        ))
    }

    fn clear_progress_interaction(&mut self) {
        self.pipeline.segment_end = None;
        self.frame.refresh_pending = false;
        self.progress.is_dragging = false;
        self.progress.pending_seek_position = None;
    }

    pub(crate) fn ensure_pipeline(&mut self, cx: &mut Context<Self>) -> anyhow::Result<bool> {
        if self.pipeline.pipeline.is_some() {
            return Ok(false);
        }
        self.pipeline.state = PlatState::Loading;
        cx.notify();
        self.set_pipeline(cx)?;
        Ok(true)
    }

    pub(crate) fn resume_or_play(&mut self, cx: &mut Context<Self>) {
        if self.resume() {
            self.start_progress_task(cx);
            self.start_frame_task(cx);
            cx.notify();
        } else {
            self.play(cx);
        }
    }

    pub(crate) fn format_time(&self, duration: Duration) -> String {
        let total_secs = duration.as_secs();
        format!("{:02}:{:02}", total_secs / 60, total_secs % 60)
    }
}
