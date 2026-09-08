use crate::{PlatState, PlayCore, PlayCoreMediaType};
use gpui_kit::Context;
use gstreamer as gst;

impl PlayCore {
    pub(crate) fn stream_metadata(
        streams: impl IntoIterator<Item = gst::Stream>,
    ) -> PlayCoreMediaType {
        let mut has_video = false;
        let mut has_audio = false;

        for stream in streams {
            let stream_type = stream.stream_type();
            has_video |= stream_type.contains(gst::StreamType::VIDEO);
            has_audio |= stream_type.contains(gst::StreamType::AUDIO);
        }

        if has_video {
            PlayCoreMediaType::Video
        } else if has_audio {
            PlayCoreMediaType::Audio
        } else {
            PlayCoreMediaType::Unknown
        }
    }

    pub(crate) fn update_stream_metadata(
        &mut self,
        media_type: PlayCoreMediaType,
        cx: &mut Context<Self>,
    ) {
        self.mark_loading_progress();
        self.set_media_type(media_type);
        self.maintain_pipeline_tasks(cx);
        cx.notify();
    }

    pub(crate) fn mark_video_present(&mut self) {
        self.set_media_type(PlayCoreMediaType::Video);
    }

    pub(crate) fn update_codec_metadata(
        &mut self,
        video_codec: Option<String>,
        audio_codec: Option<String>,
        fallback_codec: Option<String>,
    ) {
        if video_codec.is_some() {
            self.set_media_type(PlayCoreMediaType::Video);
        } else if audio_codec.is_some()
            && self.player_static_info.media_type == PlayCoreMediaType::Unknown
        {
            self.set_media_type(PlayCoreMediaType::Audio);
        }

        if let Some(codec) = video_codec.or(audio_codec).or(fallback_codec) {
            if !codec.is_empty() {
                self.player_static_info.codec = Some(codec);
            }
        }
    }

    fn set_media_type(&mut self, media_type: PlayCoreMediaType) {
        if media_type == PlayCoreMediaType::Unknown {
            return;
        }
        self.player_static_info.media_type = media_type;
        if media_type == PlayCoreMediaType::Audio {
            self.finish_audio_loading();
        }
    }

    fn finish_audio_loading(&mut self) {
        self.task.frame_task = None;
        self.task.loading_timeout_task = None;
        if self.pipeline.state == PlatState::Loading {
            self.pipeline.state = PlatState::Playing;
        }
    }
}
