use anyhow::{Context as AnyhowContext, bail};
use gpui_kit::http_client::Url;
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayCoreTranscodeFormat {
    Mp4,
    Mkv,
    MOV,
    Mp3,
    FLAC,
    WAV,
    AAC,
}

impl PlayCoreTranscodeFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Mp4 => "mp4",
            Self::Mkv => "mkv",
            Self::MOV => "mov",
            Self::Mp3 => "mp3",
            Self::FLAC => "flac",
            Self::WAV => "wav",
            Self::AAC => "aac",
        }
    }

    fn audio_encoder_factory(self) -> &'static str {
        match self {
            Self::Mp3 => "lamemp3enc",
            Self::FLAC => "flacenc",
            Self::WAV => "wavenc",
            Self::AAC => "voaacenc",
            Self::Mp4 | Self::Mkv | Self::MOV => "",
        }
    }
}

#[derive(Clone, Debug)]
pub struct PlayCoreTranscodeRequest {
    pub input: PathBuf,
    pub output: PathBuf,
    pub format: PlayCoreTranscodeFormat,
}

#[derive(Clone, Debug)]
pub struct PlayCoreRealtimeTranscodeRequest {
    pub output: PathBuf,
    pub format: PlayCoreTranscodeFormat,
    pub input_mime: Option<String>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PlayCoreTranscodeTrim {
    pub start: Duration,
    pub end: Duration,
}

pub struct PlayCoreTranscoder;

pub struct PlayCoreTranscodeSession {
    sender: mpsc::Sender<Vec<u8>>,
    result: oneshot::Receiver<anyhow::Result<()>>,
}

impl PlayCoreTranscoder {
    pub async fn transcode_offline(request: PlayCoreTranscodeRequest) -> anyhow::Result<()> {
        tokio::task::spawn_blocking(move || Self::transcode_offline_blocking(request))
            .await
            .context("转码任务异常")?
    }

    pub fn transcode_offline_blocking(request: PlayCoreTranscodeRequest) -> anyhow::Result<()> {
        if !request.input.is_file() {
            bail!("输入文件不存在: {}", request.input.display());
        }

        let input_uri = Url::from_file_path(&request.input)
            .map_err(|_| anyhow::anyhow!("无法转换输入文件路径: {}", request.input.display()))?;
        let source = make_uri_decodebin(input_uri.as_str())?;
        Self::transcode_source_blocking(source, &request.output, request.format, None)
    }

    pub(crate) fn transcode_source_blocking(
        source: gst::Element,
        output: &Path,
        format: PlayCoreTranscodeFormat,
        trim: Option<PlayCoreTranscodeTrim>,
    ) -> anyhow::Result<()> {
        gst::init().context("初始化 GStreamer 失败")?;
        prepare_output(output)?;

        let pipeline = gst::Pipeline::new();
        let branches = build_output_branches(&pipeline, output, format)?;
        connect_decodebin_pad_added(&source, branches);
        pipeline.add(&source)?;
        run_transcode_pipeline(&pipeline, trim)
    }

    pub fn start_realtime(
        request: PlayCoreRealtimeTranscodeRequest,
    ) -> anyhow::Result<PlayCoreTranscodeSession> {
        gst::init().context("初始化 GStreamer 失败")?;
        prepare_output(&request.output)?;

        let (sender, receiver) = mpsc::channel(8);
        let (result_sender, result) = oneshot::channel();
        thread::Builder::new()
            .name("player-core-realtime-transcode".to_string())
            .spawn(move || {
                let result = transcode_realtime_blocking(request, receiver);
                let _ = result_sender.send(result);
            })
            .context("启动实时转码线程失败")?;

        Ok(PlayCoreTranscodeSession { sender, result })
    }
}

impl PlayCoreTranscodeSession {
    pub async fn write_chunk(&self, chunk: impl Into<Vec<u8>>) -> anyhow::Result<()> {
        self.sender
            .send(chunk.into())
            .await
            .map_err(|_| anyhow::anyhow!("实时转码管线已结束"))
    }

    pub async fn finish(self) -> anyhow::Result<()> {
        let Self { sender, result } = self;
        drop(sender);
        result.await.context("实时转码线程异常")?
    }
}

pub(crate) fn make_uri_decodebin(uri: &str) -> anyhow::Result<gst::Element> {
    gst::init().context("初始化 GStreamer 失败")?;
    let source = gst::ElementFactory::make("uridecodebin")
        .build()
        .context("创建 GStreamer 解码源失败")?;
    source.set_property("uri", uri);
    Ok(source)
}

pub(crate) fn prepare_output(output: &Path) -> anyhow::Result<()> {
    if output.as_os_str().is_empty() {
        bail!("输出路径为空");
    }
    if output.exists() {
        bail!("输出文件已存在: {}", output.display());
    }
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("创建输出目录失败: {}", parent.display()))?;
        }
    }
    Ok(())
}

fn make_element(factory: &str) -> anyhow::Result<gst::Element> {
    gst::ElementFactory::make(factory)
        .build()
        .with_context(|| format!("GStreamer 元素不可用: {factory}"))
}

fn make_filesink(output: &Path) -> anyhow::Result<gst::Element> {
    let sink = make_element("filesink")?;
    sink.set_property("location", output.to_string_lossy().into_owned());
    Ok(sink)
}

fn add_elements(pipeline: &gst::Pipeline, elements: &[&gst::Element]) -> anyhow::Result<()> {
    for element in elements {
        pipeline.add(*element)?;
    }
    Ok(())
}

fn link_elements(elements: &[&gst::Element]) -> anyhow::Result<()> {
    for pair in elements.windows(2) {
        pair[0].link(pair[1])?;
    }
    Ok(())
}

fn sink_pad(element: &gst::Element) -> anyhow::Result<gst::Pad> {
    element.static_pad("sink").context("转码分支缺少 sink pad")
}

struct OutputBranches {
    video_sink: Option<gst::Pad>,
    audio_sink: Option<gst::Pad>,
}

fn build_output_branches(
    pipeline: &gst::Pipeline,
    output: &Path,
    format: PlayCoreTranscodeFormat,
) -> anyhow::Result<OutputBranches> {
    match format {
        PlayCoreTranscodeFormat::Mp4 | PlayCoreTranscodeFormat::MOV => {
            let video_queue = make_element("queue")?;
            let video_convert = make_element("videoconvert")?;
            let video_encoder = make_element("x264enc")?;
            let video_parser = make_element("h264parse")?;
            let audio_queue = make_element("queue")?;
            let audio_convert = make_element("audioconvert")?;
            let audio_resample = make_element("audioresample")?;
            let audio_encoder = make_element("voaacenc")?;
            let audio_parser = make_element("aacparse")?;
            let mux_factory = if matches!(format, PlayCoreTranscodeFormat::Mp4) {
                "mp4mux"
            } else {
                "qtmux"
            };
            let mux = make_element(mux_factory)?;
            if matches!(format, PlayCoreTranscodeFormat::Mp4) {
                mux.set_property("faststart", true);
            }
            let sink = make_filesink(output)?;

            let elements = vec![
                &video_queue,
                &video_convert,
                &video_encoder,
                &video_parser,
                &audio_queue,
                &audio_convert,
                &audio_resample,
                &audio_encoder,
                &audio_parser,
                &mux,
                &sink,
            ];
            add_elements(pipeline, &elements)?;

            let video_chain = vec![
                &video_queue,
                &video_convert,
                &video_encoder,
                &video_parser,
                &mux,
                &sink,
            ];
            link_elements(&video_chain)?;
            link_elements(&[
                &audio_queue,
                &audio_convert,
                &audio_resample,
                &audio_encoder,
                &audio_parser,
                &mux,
            ])?;

            Ok(OutputBranches {
                video_sink: Some(sink_pad(&video_queue)?),
                audio_sink: Some(sink_pad(&audio_queue)?),
            })
        }
        PlayCoreTranscodeFormat::Mkv => {
            let video_queue = make_element("queue")?;
            let video_convert = make_element("videoconvert")?;
            let video_encoder = make_element("x264enc")?;
            let video_parser = make_element("h264parse")?;
            let audio_queue = make_element("queue")?;
            let audio_convert = make_element("audioconvert")?;
            let audio_resample = make_element("audioresample")?;
            let audio_encoder = make_element("opusenc")?;
            let mux = make_element("matroskamux")?;
            let sink = make_filesink(output)?;

            let elements = vec![
                &video_queue,
                &video_convert,
                &video_encoder,
                &video_parser,
                &audio_queue,
                &audio_convert,
                &audio_resample,
                &audio_encoder,
                &mux,
                &sink,
            ];
            add_elements(pipeline, &elements)?;

            let video_chain = vec![
                &video_queue,
                &video_convert,
                &video_encoder,
                &video_parser,
                &mux,
                &sink,
            ];
            link_elements(&video_chain)?;
            link_elements(&[
                &audio_queue,
                &audio_convert,
                &audio_resample,
                &audio_encoder,
                &mux,
            ])?;

            Ok(OutputBranches {
                video_sink: Some(sink_pad(&video_queue)?),
                audio_sink: Some(sink_pad(&audio_queue)?),
            })
        }
        PlayCoreTranscodeFormat::Mp3
        | PlayCoreTranscodeFormat::FLAC
        | PlayCoreTranscodeFormat::WAV
        | PlayCoreTranscodeFormat::AAC => {
            let audio_queue = make_element("queue")?;
            let audio_convert = make_element("audioconvert")?;
            let audio_resample = make_element("audioresample")?;
            let audio_encoder = make_element(format.audio_encoder_factory())?;
            let audio_parser = if matches!(format, PlayCoreTranscodeFormat::AAC) {
                Some(make_element("aacparse")?)
            } else {
                None
            };
            let sink = make_filesink(output)?;

            let mut elements = vec![
                &audio_queue,
                &audio_convert,
                &audio_resample,
                &audio_encoder,
            ];
            if let Some(parser) = audio_parser.as_ref() {
                elements.push(parser);
            }
            elements.push(&sink);
            add_elements(pipeline, &elements)?;

            let mut audio_chain = vec![
                &audio_queue,
                &audio_convert,
                &audio_resample,
                &audio_encoder,
            ];
            if let Some(parser) = audio_parser.as_ref() {
                audio_chain.push(parser);
            }
            audio_chain.push(&sink);
            link_elements(&audio_chain)?;

            Ok(OutputBranches {
                video_sink: None,
                audio_sink: Some(sink_pad(&audio_queue)?),
            })
        }
    }
}

fn connect_decodebin_pad_added(source: &gst::Element, branches: OutputBranches) {
    let OutputBranches {
        video_sink,
        audio_sink,
    } = branches;
    source.connect_pad_added(move |_source, pad| {
        let caps = pad.current_caps().unwrap_or_else(|| pad.query_caps(None));
        let Some(structure) = caps.structure(0) else {
            return;
        };
        let media_type = structure.name().as_str();
        let target = if media_type.starts_with("video/") {
            video_sink.clone()
        } else if media_type.starts_with("audio/") {
            audio_sink.clone()
        } else {
            None
        };
        let Some(target) = target else {
            return;
        };
        if target.is_linked() {
            return;
        }
        if let Err(error) = pad.link(&target) {
            log::warn!("[gst:transcoder] link {media_type} pad failed: {error}");
        }
    });
}

pub(crate) fn run_pipeline_until_input_end(
    pipeline: &gst::Pipeline,
    appsrc: &gst_app::AppSrc,
    receiver: &mut tokio::sync::mpsc::Receiver<Vec<u8>>,
) -> anyhow::Result<()> {
    if let Err(error) = pipeline.set_state(gst::State::Playing) {
        let _ = pipeline.set_state(gst::State::Null);
        return Err(error.into());
    }

    let push_result = loop {
        let Some(chunk) = receiver.blocking_recv() else {
            break appsrc
                .end_of_stream()
                .map_err(|error| anyhow::anyhow!(error));
        };
        if chunk.is_empty() {
            continue;
        }
        if let Err(error) = appsrc.push_buffer(gst::Buffer::from_mut_slice(chunk)) {
            break Err(anyhow::anyhow!("向实时转码管线写入数据失败: {error}"));
        }
    };

    let result = push_result.and_then(|_| wait_for_eos(pipeline));
    let _ = pipeline.set_state(gst::State::Null);
    result
}

pub(crate) fn run_transcode_pipeline(
    pipeline: &gst::Pipeline,
    trim: Option<PlayCoreTranscodeTrim>,
) -> anyhow::Result<()> {
    let result = (|| {
        if let Some(trim) = trim {
            pipeline.set_state(gst::State::Paused)?;
            let (state_result, state, _) = pipeline.state(Some(gst::ClockTime::from_seconds(10)));
            state_result.context("转码管线进入暂停状态失败")?;
            if state < gst::State::Paused {
                bail!("转码管线未完成预加载");
            }
            let start =
                gst::ClockTime::from_nseconds(trim.start.as_nanos().min(u64::MAX as u128) as u64);
            let end =
                gst::ClockTime::from_nseconds(trim.end.as_nanos().min(u64::MAX as u128) as u64);
            pipeline.seek(
                1.0,
                gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE | gst::SeekFlags::SEGMENT,
                gst::SeekType::Set,
                start,
                gst::SeekType::Set,
                end,
            )?;
        }

        pipeline.set_state(gst::State::Playing)?;
        wait_for_eos(pipeline)
    })();
    let _ = pipeline.set_state(gst::State::Null);
    result
}

fn wait_for_eos(pipeline: &gst::Pipeline) -> anyhow::Result<()> {
    let bus = pipeline.bus().context("转码管线没有消息总线")?;
    let mut segment_finished = false;
    loop {
        let Some(message) = bus.timed_pop(gst::ClockTime::from_mseconds(100)) else {
            continue;
        };
        match message.view() {
            gst::MessageView::Eos(..) => return Ok(()),
            gst::MessageView::SegmentDone(_) if !segment_finished => {
                segment_finished = true;
                pipeline.send_event(gst::event::Eos::new());
            }
            gst::MessageView::Error(error) => {
                bail!("转码失败: {} ({:?})", error.error(), error.debug());
            }
            _ => {}
        }
    }
}

fn transcode_realtime_blocking(
    request: PlayCoreRealtimeTranscodeRequest,
    mut receiver: mpsc::Receiver<Vec<u8>>,
) -> anyhow::Result<()> {
    gst::init().context("初始化 GStreamer 失败")?;

    let pipeline = gst::Pipeline::new();
    let appsrc = gst_app::AppSrc::builder()
        .stream_type(gst_app::AppStreamType::Stream)
        .format(gst::Format::Bytes)
        .is_live(false)
        .block(true)
        .max_bytes(4 * 1024 * 1024)
        .automatic_eos(false)
        .build();
    if let Some(mime) = request.input_mime.as_deref() {
        appsrc.set_caps(Some(&gst::Caps::builder(mime).build()));
    }

    let decodebin = make_element("decodebin")?;
    let appsrc_element = appsrc.upcast_ref::<gst::Element>();
    pipeline.add(appsrc_element)?;
    pipeline.add(&decodebin)?;
    appsrc_element.link(&decodebin)?;

    let branches = build_output_branches(&pipeline, &request.output, request.format)?;
    connect_decodebin_pad_added(&decodebin, branches);
    run_pipeline_until_input_end(&pipeline, &appsrc, &mut receiver)
}
