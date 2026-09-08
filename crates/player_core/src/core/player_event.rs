use crate::state::{PlayCoreGlobalState, PlayCoreStateEvent};
use crate::{PlatState, PlayCore, PlayCoreMediaType};
use gpui_kit::*;
use gstreamer as gst;
use gstreamer::prelude::*;
use std::time::Duration;

impl PlayCore {
    pub(crate) fn start_event_bus(&mut self, cx: &mut Context<Self>) {
        if self.task.bus_watch_task.is_some() {
            return;
        }
        let Some(pipeline) = self.pipeline.pipeline.clone() else {
            return;
        };
        let Some(bus) = pipeline.bus() else {
            return;
        };

        let is_local_file = self.player_static.url.starts_with("file://");
        let session_id = self.pipeline.session_id;
        self.task.bus_watch_task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;

                let current = this
                    .update(cx, |this, _| this.pipeline.is_current_session(session_id))
                    .unwrap_or(false);
                if !current {
                    break;
                }

                let mut stop_loop = false;
                while let Some(message) = bus.timed_pop(gst::ClockTime::from_mseconds(0)) {
                    match message.view() {
                        gst::MessageView::Error(error) => {
                            log::info!(
                                "[gst:error] source={} error={} debug={:?}",
                                message
                                    .src()
                                    .map(|src| src.path_string())
                                    .unwrap_or_else(|| "unknown".into()),
                                error.error(),
                                error.debug()
                            );
                            let _ = this.update(cx, |this, cx| {
                                if !this.pipeline.is_current_session(session_id) {
                                    return;
                                }
                                this.reset_pipeline();
                                this.pipeline.state = PlatState::Error("播放失败".to_string());
                                cx.notify();
                            });
                            stop_loop = true;
                            break;
                        }
                        gst::MessageView::Warning(warning) => {
                            log::info!(
                                "[gst:warning] source={} warning={} debug={:?}",
                                message
                                    .src()
                                    .map(|src| src.path_string())
                                    .unwrap_or_else(|| "unknown".into()),
                                warning.error(),
                                warning.debug()
                            );
                        }
                        gst::MessageView::StreamCollection(collection) => {
                            let metadata =
                                PlayCore::stream_metadata(&collection.stream_collection());
                            let _ = this.update(cx, |this, cx| {
                                if this.pipeline.is_current_session(session_id) {
                                    this.update_stream_metadata(metadata, cx);
                                }
                            });
                        }
                        gst::MessageView::StreamsSelected(selected) => {
                            let metadata = PlayCore::stream_metadata(selected.streams());
                            let _ = this.update(cx, |this, cx| {
                                if this.pipeline.is_current_session(session_id) {
                                    this.update_stream_metadata(metadata, cx);
                                }
                            });
                        }
                        gst::MessageView::Tag(tag) => {
                            let tags = tag.tags();
                            let video_codec = tags
                                .get::<gst::tags::VideoCodec>()
                                .map(|value| value.get().to_string())
                                .filter(|codec| !codec.is_empty());
                            let audio_codec = tags
                                .get::<gst::tags::AudioCodec>()
                                .map(|value| value.get().to_string())
                                .filter(|codec| !codec.is_empty());
                            let fallback_codec = tags
                                .get::<gst::tags::Codec>()
                                .map(|value| value.get().to_string())
                                .filter(|codec| !codec.is_empty());
                            if video_codec.is_none()
                                && audio_codec.is_none()
                                && fallback_codec.is_none()
                            {
                                continue;
                            }
                            let _ = this.update(cx, |this, cx| {
                                if this.pipeline.is_current_session(session_id) {
                                    this.mark_loading_progress();
                                    this.update_codec_metadata(
                                        video_codec,
                                        audio_codec,
                                        fallback_codec,
                                    );
                                    this.maintain_pipeline_tasks(cx);
                                    cx.notify();
                                }
                            });
                        }
                        gst::MessageView::SegmentDone(_) => {
                            let _ = pipeline.set_state(gst::State::Paused);
                            let _ = this.update(cx, |this, cx| {
                                if !this.pipeline.is_current_session(session_id) {
                                    return;
                                }
                                if let Some(end) = this.pipeline.segment_end.take() {
                                    this.progress.position = end;
                                }
                                this.pipeline.buffering_paused = false;
                                this.pipeline.state = PlatState::Paused;
                                cx.notify();
                            });
                        }
                        gst::MessageView::Buffering(buffering) if !is_local_file => {
                            let percent = buffering.percent();
                            if percent < 100 {
                                let paused = pipeline.set_state(gst::State::Paused).is_ok();
                                let _ = this.update(cx, |this, cx| {
                                    if this.pipeline.is_current_session(session_id) {
                                        this.mark_buffering_progress(percent);
                                        if paused
                                            && matches!(
                                                &this.pipeline.state,
                                                PlatState::Loading | PlatState::Playing
                                            )
                                        {
                                            this.pipeline.buffering_paused = true;
                                            this.pipeline.state = PlatState::Loading;
                                        }
                                        cx.notify();
                                    }
                                });
                            } else {
                                let should_resume = this
                                    .update(cx, |this, cx| {
                                        if !this.pipeline.is_current_session(session_id) {
                                            return None;
                                        }
                                        this.mark_buffering_progress(percent);
                                        if this.pipeline.state == PlatState::Paused
                                            || !matches!(
                                                &this.pipeline.state,
                                                PlatState::Loading | PlatState::Playing
                                            )
                                        {
                                            cx.notify();
                                            return None;
                                        }
                                        this.pipeline.buffering_paused = false;
                                        Some(())
                                    })
                                    .unwrap_or(None);
                                if should_resume.is_some() {
                                    let resumed = pipeline.set_state(gst::State::Playing).is_ok();
                                    let _ = this.update(cx, |this, cx| {
                                        if this.pipeline.is_current_session(session_id) {
                                            if resumed {
                                                this.pipeline.state =
                                                    if this.player_static_info.media_type
                                                        == PlayCoreMediaType::Audio
                                                    {
                                                        PlatState::Playing
                                                    } else {
                                                        PlatState::Loading
                                                    };
                                            } else {
                                                this.reset_pipeline();
                                                this.pipeline.state =
                                                    PlatState::Error("播放失败".to_string());
                                            }
                                            cx.notify();
                                        }
                                    });
                                }
                            }
                        }
                        gst::MessageView::Latency(_) => {
                            if let Ok(bin) = pipeline.clone().dynamic_cast::<gst::Bin>() {
                                let _ = bin.recalculate_latency();
                            }
                        }
                        gst::MessageView::Eos(_) => {
                            let _ = this.update(cx, |this, cx| {
                                if !this.pipeline.is_current_session(session_id) {
                                    return;
                                }
                                PlayCoreGlobalState::publish(
                                    cx,
                                    PlayCoreStateEvent::PlayBackFished(
                                        this.window_id,
                                        cx.entity_id(),
                                        this.player_static.clone(),
                                    ),
                                );
                                this.finish_pipeline();
                                cx.notify();
                            });
                            stop_loop = true;
                            break;
                        }
                        _ => {}
                    }
                }

                if stop_loop {
                    break;
                }
                let keep_running = this
                    .update(cx, |this, _| {
                        this.pipeline.is_current_session(session_id)
                            && this.pipeline.pipeline.is_some()
                    })
                    .unwrap_or(false);
                if !keep_running {
                    break;
                }
            }
        }));
    }
}
