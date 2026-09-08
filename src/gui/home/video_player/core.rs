use super::ExportStatus;
use super::SidePanelState;
use super::VideoPlayer;
use anyhow::Context as AnyhowContext;
use gpui_kit::http_client::Url;
use gpui_kit::{AsyncApp, Context, EntityId, Window};
use player_core::{
    PlayCoreDownload, PlayCoreDownloadRequest, PlayCoreTranscodeFormat, PlayCoreTranscodeRequest,
    PlayCoreTranscoder, PlayStatic,
};
use reqwest::header::HeaderMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use uuid::Uuid;

impl VideoPlayer {
    pub(super) fn toggle_side_panel(&mut self, cx: &mut Context<Self>) {
        let next_state = match self.side_panel_state {
            SidePanelState::Open | SidePanelState::Opening => SidePanelState::Closing,
            SidePanelState::Closed | SidePanelState::Closing => SidePanelState::Opening,
        };
        self.side_panel_state = next_state;
        self.side_panel_animation_id = self.side_panel_animation_id.wrapping_add(1);
        let animation_id = self.side_panel_animation_id;

        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(500))
                .await;
            this.update(cx, |this, cx| {
                if this.side_panel_animation_id != animation_id {
                    return;
                }

                this.side_panel_state = match next_state {
                    SidePanelState::Opening => SidePanelState::Open,
                    SidePanelState::Closing => SidePanelState::Closed,
                    SidePanelState::Open | SidePanelState::Closed => return,
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub fn _play_core_id(&self) -> EntityId {
        self.play_core.entity_id()
    }

    pub fn _set_play_list(&mut self, play_list: Vec<PlayStatic>, cx: &mut Context<Self>) {
        let current_id = self
            .current_index
            .and_then(|index| self.play_list.get(index))
            .map(|item| item.id.clone());
        self.play_list = play_list;
        self.play_list_state.reset(self.play_list.len());
        self.current_index =
            current_id.and_then(|id| self.play_list.iter().position(|item| item.id == id));
        cx.notify();
    }

    pub fn _append_play_item(&mut self, item: PlayStatic, cx: &mut Context<Self>) {
        let index = self.play_list.len();
        self.play_list.push(item);
        self.play_list_state.splice(index..index, 1);
        cx.notify();
    }

    pub(super) fn _append_and_play(&mut self, item: PlayStatic, cx: &mut Context<Self>) {
        self._append_play_item(item, cx);
        self._play_item(self.play_list.len() - 1, cx);
    }

    pub(super) fn _play_url(&mut self, url: String, cx: &mut Context<Self>) {
        let title = url
            .rsplit('/')
            .next()
            .and_then(|part| part.split(['?', '#']).next())
            .filter(|part| !part.trim().is_empty())
            .unwrap_or("网络媒体")
            .to_string();
        self._append_and_play(
            PlayStatic {
                id: Uuid::new_v4().to_string(),
                title,
                url,
                headers: HeaderMap::new(),
            },
            cx,
        );
    }

    pub(super) fn _play_network_url(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let url = self.network_url_input.read(cx).value().trim().to_string();
        if url.is_empty() {
            return;
        }

        self._play_url(url, cx);
        self.network_url_input
            .update(cx, |input, cx| input.set_value("", window, cx));
    }

    pub(super) fn export_media(&mut self, format: PlayCoreTranscodeFormat, cx: &mut Context<Self>) {
        let source = self.play_core.read(cx)._view_state().player;
        if source.url.trim().is_empty() {
            self.export_status = Some(ExportStatus::Error("暂无可导出的媒体".to_string()));
            cx.notify();
            return;
        }
        if self.export_in_progress {
            return;
        }

        let suggested_name = suggested_export_name(&source.title, format);
        let save_dialog = cx.prompt_for_new_path(&PathBuf::default(), Some(&suggested_name));
        let entity = cx.entity().clone();
        let mut cx_async = cx.to_async().clone();
        let format_label = export_format_label(format).to_string();

        self.export_in_progress = true;
        self.export_status = Some(ExportStatus::Working(format!(
            "正在准备导出 {format_label}..."
        )));
        cx.notify();

        cx.spawn(move |_, _: &mut AsyncApp| async move {
            let path = match save_dialog.await {
                Ok(Ok(Some(path))) => path,
                Ok(Ok(None)) => {
                    let _ = entity.update(&mut cx_async, |this, cx| {
                        this.export_in_progress = false;
                        this.export_status = None;
                        cx.notify();
                    });
                    return;
                }
                Ok(Err(error)) => {
                    let _ = entity.update(&mut cx_async, |this, cx| {
                        this.finish_export(
                            Err(anyhow::anyhow!("打开保存对话框失败: {error:#}")),
                            cx,
                        );
                    });
                    return;
                }
                Err(error) => {
                    let _ = entity.update(&mut cx_async, |this, cx| {
                        this.finish_export(Err(anyhow::anyhow!("保存路径选择失败: {error:#}")), cx);
                    });
                    return;
                }
            };

            let output = path.with_extension(export_format_extension(format));
            let result = tokio::spawn(run_export(source, output.clone(), format))
                .await
                .map_err(|error| anyhow::anyhow!("导出任务异常: {error}"))
                .and_then(|result| result.map(|_| output));

            let _ = entity.update(&mut cx_async, |this, cx| {
                this.finish_export(result, cx);
            });
        })
        .detach();
    }

    fn finish_export(&mut self, result: anyhow::Result<PathBuf>, cx: &mut Context<Self>) {
        self.export_in_progress = false;
        self.export_status = Some(match result {
            Ok(path) => ExportStatus::Success(format!("导出完成：{}", path.display())),
            Err(error) => ExportStatus::Error(format!("导出失败：{error:#}")),
        });
        cx.notify();
    }

    pub fn _play_item(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(item) = self.play_list.get(index).cloned() else {
            return;
        };
        self.current_index = Some(index);
        let _ = self
            .play_core
            .update(cx, |player, cx| player._play_source(item, cx));
        cx.notify();
    }

    pub(crate) fn _play_previous(&mut self, cx: &mut Context<Self>) {
        let Some(index) = self.current_index else {
            return;
        };
        if let Some(previous) = index.checked_sub(1) {
            self._play_item(previous, cx);
        }
    }

    pub(crate) fn _play_next(&mut self, cx: &mut Context<Self>) {
        let Some(index) = self.current_index else {
            return;
        };
        if index + 1 < self.play_list.len() {
            self._play_item(index + 1, cx);
        }
    }
}

async fn run_export(
    source: PlayStatic,
    output: PathBuf,
    format: PlayCoreTranscodeFormat,
) -> anyhow::Result<()> {
    if source.url.starts_with("file://") {
        let input = file_path_from_url(&source.url)?;
        return PlayCoreTranscoder::transcode_offline(PlayCoreTranscodeRequest {
            input,
            output,
            format,
        })
        .await;
    }

    PlayCoreDownload::download(PlayCoreDownloadRequest {
        url: source.url,
        headers: source.headers,
        output,
        format,
    })
    .await
}

fn file_path_from_url(url: &str) -> anyhow::Result<PathBuf> {
    let url = Url::parse(url).with_context(|| format!("无法解析本地媒体地址: {url}"))?;
    url.to_file_path()
        .map_err(|_| anyhow::anyhow!("无法转换本地媒体路径: {url}"))
}

pub(super) fn export_format_label(format: PlayCoreTranscodeFormat) -> &'static str {
    match format {
        PlayCoreTranscodeFormat::Mp4 => "MP4 · H.264 + AAC",
        PlayCoreTranscodeFormat::Mkv => "MKV · H.264 + Opus",
        PlayCoreTranscodeFormat::MOV => "MOV · H.264 + AAC",
        PlayCoreTranscodeFormat::Mp3 => "MP3 音频",
        PlayCoreTranscodeFormat::FLAC => "FLAC 音频",
        PlayCoreTranscodeFormat::WAV => "WAV 音频",
        PlayCoreTranscodeFormat::AAC => "AAC 音频",
    }
}

fn export_format_extension(format: PlayCoreTranscodeFormat) -> &'static str {
    format.extension()
}

fn suggested_export_name(title: &str, format: PlayCoreTranscodeFormat) -> String {
    let title = title.rsplit(['/', '\\']).next().unwrap_or(title).trim();
    let stem = Path::new(title)
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or(title);
    let safe_stem = stem
        .chars()
        .map(|character| {
            if matches!(
                character,
                '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
            ) {
                '_'
            } else {
                character
            }
        })
        .collect::<String>();
    let stem = if safe_stem.trim().is_empty() {
        "导出媒体"
    } else {
        safe_stem.trim()
    };
    format!("{stem}.{}", export_format_extension(format))
}
