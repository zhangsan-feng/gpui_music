use super::FetchDocument;
use crate::drive::{NetworkStatic, NetworkStaticInterface};
use crate::plugins::extractor::config::{self, PlatformConfig};
#[cfg(test)]
use futures_util::future::join_all;
use gpui_kit::http_client::Url;

#[cfg(test)]
pub(crate) async fn filter_playable(items: Vec<NetworkStatic>) -> Vec<NetworkStatic> {
    join_all(items.into_iter().map(|item| async move {
        let candidate = item.clone();
        let playable = tokio::task::spawn_blocking(move || {
            let details = candidate.func.detail(&candidate);
            details.iter().any(|detail| {
                let source = detail.func.play(detail);
                !source.trim().is_empty()
            })
        })
        .await
        .unwrap_or(false);
        playable.then_some(item)
    }))
    .await
    .into_iter()
    .flatten()
    .collect()
}

pub(crate) struct ConfiguredVideoInterface {
    pub(crate) config: PlatformConfig,
    pub(crate) fetcher: FetchDocument,
}

impl NetworkStaticInterface for ConfiguredVideoInterface {
    fn download(&self, _params: &NetworkStatic) {}

    fn play(&self, params: &NetworkStatic) -> String {
        if params.source.trim().is_empty() {
            // log::warn!("[video:play] empty source, id={}", params.id);
            return String::new();
        }
        if is_direct_media_source(&params.source) {
            return params.source.clone();
        }

        let Some(detail) = self.config.item_children.detail.as_ref() else {
            // log::info!(
            //     "[video:play] no detail resolver, use source as-is id={} url={}",
            //     params.id,
            //     params.source
            // );
            return unwrap_play_source(&params.source);
        };
        let Some(play) = detail.play.as_ref() else {
            // log::info!(
            //     "[video:play] no play resolver, use source as-is id={} url={}",
            //     params.id,
            //     params.source
            // );
            return unwrap_play_source(&params.source);
        };
        let url = play
            .base_url
            .as_deref()
            .map(|template| config::resolve_template(&params.source, template, &params.source))
            .unwrap_or_else(|| params.source.clone());
        let config = self.config.clone();
        let fetcher = self.fetcher.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let extract_type = config::play_extract_type(&config, play);
                let Ok(document) = fetcher(url.clone(), config.clone(), extract_type).await else {
                    log::error!("video play request failed: url={url}");
                    return String::new();
                };
                let source =
                    config::extract_play_url(&document, play, &url, &config).unwrap_or_default();
                if source.trim().is_empty() {
                    log::warn!("[video:play] resolver returned empty source: url={url}");
                } else {
                    log::info!("[video:play] resolved url={source} from={url}");
                }
                source
            })
        })
    }

    fn detail(&self, params: &NetworkStatic) -> Vec<NetworkStatic> {
        if params.source.contains("/vod/play") || params.source.contains("vodplay") {
            return vec![params.clone()];
        }
        let Some(detail) = self.config.item_children.detail.as_ref() else {
            return vec![params.clone()];
        };
        let Some(children) = detail.item_children.as_ref() else {
            return vec![params.clone()];
        };
        let detail_url = detail
            .base_url
            .as_deref()
            .map(|template| {
                config::resolve_template(&self.config.base_url, template, &params.source)
            })
            .unwrap_or_else(|| params.source.clone());
        let child_url = children
            .base_url
            .as_deref()
            .map(|template| config::resolve_template(&detail_url, template, &params.source))
            .unwrap_or_else(|| detail_url.clone());
        let config = self.config.clone();
        let fetcher = self.fetcher.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let extract_type = config::detail_extract_type(&config, detail, children);
                let Ok(document) = fetcher(child_url.clone(), config.clone(), extract_type).await
                else {
                    log::error!("video detail request failed: url={child_url}");
                    return Vec::new();
                };
                let values = config::parse_items(&document, children, &child_url);
                if values.is_empty() {
                    log::warn!("video detail returned no episodes: url={child_url}");
                    return Vec::new();
                }
                // log::info!("[video:detail] url={} episodes={}", child_url, values.len());
                values
                    .into_iter()
                    .map(|item| NetworkStatic {
                        id: uuid::Uuid::new_v4().to_string(),
                        name: item.name,
                        img: if item.image.is_empty() {
                            params.img.clone()
                        } else {
                            item.image
                        },
                        author: params.author.clone(),
                        category: params.category.clone(),
                        headers: params.headers.clone(),
                        extra: item.extra,
                        source: item.source,
                        func: params.func.clone(),
                    })
                    .collect()
            })
        })
    }
}

fn unwrap_play_source(source: &str) -> String {
    let Some(url) = Url::parse(source).ok() else {
        return source.to_string();
    };
    let Some(embedded_url) = url.query_pairs().find_map(|(_, value)| {
        let value = value.trim();
        (value.starts_with("http://") || value.starts_with("https://")).then(|| value.to_string())
    }) else {
        return source.to_string();
    };
    // log::info!(
    //     "[video:play] unwrapped embedded source from={} to={}",
    //     source,
    //     embedded_url
    // );
    embedded_url
}

fn is_direct_media_source(source: &str) -> bool {
    let Ok(url) = Url::parse(source) else {
        return false;
    };
    let path = url.path().to_ascii_lowercase();
    path.ends_with(".m3u8") || path.ends_with(".mp4")
}
