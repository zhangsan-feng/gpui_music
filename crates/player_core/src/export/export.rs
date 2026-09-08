use crate::transcoder::transcoder::{
    PlayCoreTranscodeFormat, PlayCoreTranscodeTrim, PlayCoreTranscoder, make_uri_decodebin,
};
use anyhow::{Context as AnyhowContext, bail};
use gpui_kit::http_client::Url;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayCoreExportTrim {
    pub start: Duration,
    pub end: Duration,
}

#[derive(Clone, Debug)]
pub struct PlayCoreExportRequest {
    pub input: PathBuf,
    pub output: PathBuf,
    pub format: PlayCoreTranscodeFormat,
    pub trim: Option<PlayCoreExportTrim>,
}

pub struct PlayCoreExport;

impl PlayCoreExport {
    pub async fn export(request: PlayCoreExportRequest) -> anyhow::Result<()> {
        tokio::task::spawn_blocking(move || Self::export_blocking(request))
            .await
            .context("导出任务异常")?
    }

    pub fn export_blocking(request: PlayCoreExportRequest) -> anyhow::Result<()> {
        validate_request(&request)?;

        let input_uri = Url::from_file_path(&request.input)
            .map_err(|_| anyhow::anyhow!("无法转换输入文件路径: {}", request.input.display()))?
            .to_string();
        let source = make_uri_decodebin(&input_uri)?;
        let trim = request.trim.map(|trim| PlayCoreTranscodeTrim {
            start: trim.start,
            end: trim.end,
        });
        PlayCoreTranscoder::transcode_source_blocking(source, &request.output, request.format, trim)
    }
}

fn validate_request(request: &PlayCoreExportRequest) -> anyhow::Result<()> {
    if !request.input.is_file() {
        bail!("输入文件不存在: {}", request.input.display());
    }
    if let Some(trim) = request.trim {
        if trim.start >= trim.end {
            bail!("导出裁剪区间无效: start 必须小于 end");
        }
    }
    Ok(())
}
