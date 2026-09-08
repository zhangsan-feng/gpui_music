use crate::{PlayCore, PlayCoreMediaType};
use anyhow::Context as AnyhowContext;
use gpui_kit::Context;
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use gstreamer_video as gst_video;

impl Drop for PlayCore {
    fn drop(&mut self) {
        self.reset_pipeline_state();
    }
}

impl PlayCore {
    pub(crate) fn set_pipeline(&mut self, cx: &mut Context<Self>) -> anyhow::Result<()> {
        if self.pipeline.pipeline.is_some() {
            return Ok(());
        }

        gst::init().context("初始化 GStreamer 失败")?;
        let playbin = gst::ElementFactory::make("playbin3")
            .name("video-playbin")
            .build()?;
        let mut headers = self.player_static.headers.clone();
        if !headers.contains_key(reqwest::header::USER_AGENT) {
            headers.insert(
                reqwest::header::USER_AGENT,
                reqwest::header::HeaderValue::from_static(
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/131.0.0.0 Safari/537.36",
                ),
            );
        }
        log::info!(
            "[gst:play] uri={} headers={:?}",
            self.player_static.url,
            headers.keys().map(|name| name.as_str()).collect::<Vec<_>>()
        );
        playbin.connect("source-setup", false, move |values| {
            let Some(source) = values
                .get(1)
                .and_then(|value| value.get::<gst::Element>().ok())
            else {
                return None;
            };

            let has_extra_headers = source.find_property("extra-headers").is_some();
            let has_user_agent = source.find_property("user-agent").is_some();
            if !has_extra_headers && !has_user_agent {
                return None;
            }

            let mut extra_headers = gst::Structure::builder("extra-headers");
            let mut header_count = 0;
            let mut applied_headers = Vec::new();
            for (name, value) in &headers {
                let Ok(value) = value.to_str() else {
                    continue;
                };
                if name == reqwest::header::USER_AGENT {
                    if has_user_agent {
                        source.set_property("user-agent", value);
                        applied_headers.push(name.as_str());
                    }
                    continue;
                }
                if has_extra_headers {
                    extra_headers = extra_headers.field(name.as_str(), value.to_owned());
                    header_count += 1;
                    applied_headers.push(name.as_str());
                }
            }

            if has_extra_headers && header_count > 0 {
                source.set_property("extra-headers", extra_headers.build());
            }
            log::info!(
                "[gst:source-setup] source={} headers={:?}",
                source.name(),
                applied_headers
            );
            None
        });

        let caps = gst::Caps::builder("video/x-raw")
            .field("format", "BGRA")
            .build();
        let buffer_clone = self.frame.images.latest_frame();
        let appsink = gst_app::AppSink::builder()
            .caps(&caps)
            .sync(true)
            .max_buffers(3)
            .drop(true)
            .callbacks(
                gst_app::AppSinkCallbacks::builder()
                    .new_sample(move |appsink| {
                        let sample = match appsink.pull_sample() {
                            Ok(sample) => sample,
                            Err(_) => return Ok(gst::FlowSuccess::Ok),
                        };
                        let caps = match sample.caps() {
                            Some(caps) => caps,
                            None => return Ok(gst::FlowSuccess::Ok),
                        };
                        let info = match gst_video::VideoInfo::from_caps(&caps) {
                            Ok(info) => info,
                            Err(_) => return Ok(gst::FlowSuccess::Ok),
                        };
                        let width = info.width() as usize;
                        let height = info.height() as usize;
                        if width == 0 || height == 0 {
                            return Ok(gst::FlowSuccess::Ok);
                        }
                        let fps = info.fps();
                        let frame_rate = if fps.denom() > 0 {
                            fps.numer() as f64 / fps.denom() as f64
                        } else {
                            0.0
                        };
                        let Some(buffer) = sample.buffer() else {
                            return Ok(gst::FlowSuccess::Ok);
                        };
                        let Ok(map) = buffer.map_readable() else {
                            return Ok(gst::FlowSuccess::Ok);
                        };
                        let Some(stride) = info.stride().first().copied() else {
                            return Ok(gst::FlowSuccess::Ok);
                        };
                        let Some(row_bytes) = width.checked_mul(4) else {
                            return Ok(gst::FlowSuccess::Ok);
                        };
                        if stride <= 0 || (stride as usize) < row_bytes {
                            return Ok(gst::FlowSuccess::Ok);
                        }
                        let stride = stride as usize;
                        let data = map.as_slice();
                        let Some(required_bytes) = stride.checked_mul(height) else {
                            return Ok(gst::FlowSuccess::Ok);
                        };
                        let Some(output_bytes) = row_bytes.checked_mul(height) else {
                            return Ok(gst::FlowSuccess::Ok);
                        };
                        if data.len() < required_bytes {
                            return Ok(gst::FlowSuccess::Ok);
                        }

                        let mut out = vec![0u8; output_bytes];
                        if stride == row_bytes {
                            out.copy_from_slice(&data[..row_bytes * height]);
                        } else {
                            for y in 0..height {
                                let src_start = y * stride;
                                let dst_start = y * row_bytes;
                                out[dst_start..dst_start + row_bytes]
                                    .copy_from_slice(&data[src_start..src_start + row_bytes]);
                            }
                        }

                        let mut target = buffer_clone.lock().unwrap();
                        target.info.width = width as u32;
                        target.info.height = height as u32;
                        target.info.frame_rate = frame_rate;
                        target.data = out;
                        target.seq = target.seq.wrapping_add(1);
                        Ok(gst::FlowSuccess::Ok)
                    })
                    .build(),
            )
            .build();

        playbin.set_property("video-sink", &appsink);
        playbin.set_property("volume", &(self.volume.value as f64));
        playbin.set_property("uri", &self.player_static.url);
        playbin.set_state(gst::State::Paused)?;

        self.pipeline.pipeline = Some(playbin);
        self.start_event_bus(cx);
        self.start_loading_timeout_task(cx);
        Ok(())
    }

    pub(crate) fn set_playing(&self) -> bool {
        self.set_pipeline_state(gst::State::Playing)
    }

    pub(crate) fn set_paused(&mut self) -> bool {
        if self.pipeline.pipeline.is_none() {
            return false;
        }
        if !self.set_pipeline_state(gst::State::Paused) {
            return false;
        }
        self.task.loading_timeout_task = None;
        true
    }

    pub(crate) fn maintain_pipeline_tasks(&mut self, cx: &mut Context<Self>) {
        self.start_loading_timeout_task(cx);
        if self.player_static_info.media_type == PlayCoreMediaType::Video {
            self.start_frame_task(cx);
        }
    }

    pub(crate) fn mark_loading_progress(&mut self) {
        self.pipeline.loading_progress = self.pipeline.loading_progress.wrapping_add(1);
    }

    pub(crate) fn mark_buffering_progress(&mut self, percent: i32) {
        if self.pipeline.last_buffering_percent == Some(percent) {
            return;
        }
        self.pipeline.last_buffering_percent = Some(percent);
        self.mark_loading_progress();
    }

    pub(crate) fn reset_pipeline_state(&mut self) {
        if let Some(pipeline) = &self.pipeline.pipeline {
            let _ = pipeline.set_state(gst::State::Null);
        }
        self.pipeline.pipeline = None;
    }

    fn set_pipeline_state(&self, state: gst::State) -> bool {
        self.pipeline
            .pipeline
            .as_ref()
            .map(|pipeline| pipeline.set_state(state).is_ok())
            .unwrap_or(false)
    }

    pub(crate) fn seek_pipeline(&mut self, position: std::time::Duration) -> bool {
        let Some(pipeline) = &self.pipeline.pipeline else {
            return false;
        };
        let target = PlayCore::duration_to_clock_time(position);
        let seeked = pipeline
            .seek_simple(gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE, target)
            .is_ok();
        seeked
    }

    pub(crate) fn seek_segment_pipeline(
        &self,
        start: std::time::Duration,
        end: std::time::Duration,
    ) -> bool {
        let Some(pipeline) = &self.pipeline.pipeline else {
            return false;
        };
        let start = PlayCore::duration_to_clock_time(start);
        let end = PlayCore::duration_to_clock_time(end);
        pipeline
            .seek(
                1.0,
                gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE | gst::SeekFlags::SEGMENT,
                gst::SeekType::Set,
                start,
                gst::SeekType::Set,
                end,
            )
            .is_ok()
    }

    pub(crate) fn set_pipeline_volume(&self, volume: f32) {
        if let Some(pipeline) = &self.pipeline.pipeline {
            pipeline.set_property("volume", &(volume as f64));
        }
    }
}
