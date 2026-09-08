use super::{DetailOrigin, Page, VideoPage};
use crate::drive::NetworkStatic;
use crate::plugins::extractor::video;
use gpui_kit::{AsyncApp, Context, EntityId, Window, WindowId, point, px};
use log::info;
use player_core::{PlayCore, PlayCoreGlobalState, PlayCoreStateEvent, PlayStatic};

impl VideoPage {
    pub(super) fn init_data(&mut self, cx: &mut Context<Self>) {
        let mut cx_async = cx.to_async().clone();
        let entity = cx.entity().clone();
        self.is_loading = true;

        cx.spawn(move |_, _: &mut AsyncApp| async move {
            let res = tokio::spawn(async move { video::recommend().await });
            match res.await {
                Ok(result) => {
                    let _ = entity.update(&mut cx_async, |this, cx| {
                        this.recommend_result = result;
                        this.is_loading = false;
                        cx.notify();
                    });
                }
                Err(error) => {
                    let _ = entity.update(&mut cx_async, |this, cx| {
                        this.is_loading = false;
                        cx.notify();
                    });
                    log::error!("video recommend task failed: {error}");
                }
            }
        })
        .detach();
    }

    pub(super) fn search_video(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let mut cx_async = cx.to_async().clone();
        let entity = cx.entity().clone();
        let search_keyword = self.search_keyword.read(cx).value().to_string();

        self.is_loading = true;
        self.is_searching = true;
        self.current_page = Page::Search;
        self.search_result.clear();
        cx.notify();

        cx.spawn(|_, _: &mut AsyncApp| async move {
            let res = tokio::spawn(async move { video::search(search_keyword).await });
            match res.await {
                Ok(result) => {
                    let _ = entity.update(&mut cx_async, |this, cx| {
                        this.search_result = result;
                        this.is_loading = false;
                        this.is_searching = false;
                        cx.notify();
                    });
                }
                Err(error) => {
                    let _ = entity.update(&mut cx_async, |this, cx| {
                        this.is_loading = false;
                        this.is_searching = false;
                        cx.notify();
                    });
                    log::error!("video search task failed: {error}");
                }
            }
        })
        .detach();
    }

    pub(super) fn clear_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.current_page = Page::Recommend;
        self.is_searching = false;
        self.search_result.clear();
        self.search_keyword
            .update(cx, |input, cx| input.set_value("", window, cx));
        cx.notify();
    }

    pub(super) fn open_detail(
        &mut self,
        data: NetworkStatic,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.detail_origin = match self.current_page {
            Page::Search => DetailOrigin::Search,
            _ => DetailOrigin::Recommend,
        };
        self.current_page = Page::Detail;
        self.detail_source = Some(data.clone());
        self.detail_result.clear();
        self.active_player_target = None;
        self.detail_scroll_handler.set_offset(point(px(0.), px(0.)));
        self.is_detail_loading = true;
        cx.notify();

        let source_id = data.id.clone();
        let entity = cx.entity().clone();
        let mut cx_async = cx.to_async().clone();
        cx.spawn(|_, _: &mut AsyncApp| async move {
            let mut result = tokio::spawn(async move { data.func.detail(&data) })
                .await
                .unwrap_or_default();
            super::detail_page::sort_episodes(&mut result);

            let _ = entity.update(&mut cx_async, |this, cx| {
                let is_current_detail = this.current_page == Page::Detail
                    && this
                        .detail_source
                        .as_ref()
                        .is_some_and(|source| source.id == source_id);
                if !is_current_detail {
                    return;
                }

                this.detail_result = result;
                this.is_detail_loading = false;
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn back_from_detail(&mut self, cx: &mut Context<Self>) {
        self.current_page = match self.detail_origin {
            DetailOrigin::Recommend => Page::Recommend,
            DetailOrigin::Search => Page::Search,
        };
        self.detail_source = None;
        self.detail_result.clear();
        self.active_player_target = None;
        self.detail_scroll_handler.set_offset(point(px(0.), px(0.)));
        self.is_detail_loading = false;
        cx.notify();
    }

    pub(super) fn play_episode(
        &mut self,
        episode: NetworkStatic,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let window_title = format!("{} - {}", episode.name, episode.source);
        let (player_window_id, player_entity_id) =
            match PlayCore::_open_window(window, cx, &window_title) {
                Ok(target) => target,
                Err(error) => {
                    log::error!("创建播放器窗口失败: {error:#}");
                    return;
                }
            };
        self.active_player_target = Some((player_window_id, player_entity_id));
        self.request_play_episode(episode, player_window_id, player_entity_id, cx);
    }

    pub(super) fn request_play_episode(
        &self,
        episode: NetworkStatic,
        player_window_id: WindowId,
        player_entity_id: EntityId,
        cx: &mut Context<Self>,
    ) {
        let mut cx_async = cx.to_async().clone();

        cx.spawn(move |_, _: &mut AsyncApp| async move {
            let episode_for_play = episode.clone();
            let source = tokio::spawn(async move { episode_for_play.func.play(&episode_for_play) })
                .await
                .unwrap_or_default();

            if source.trim().is_empty() {
                info!(
                    "[video:play-request] no playable source episode_id={} source={}",
                    episode.id, episode.source
                );
                return;
            }

            info!(
                "[video:play-request] episode_id={} url={} window_id={} entity_id={} headers={}",
                episode.id,
                source,
                player_window_id.as_u64(),
                player_entity_id,
                episode.headers.len()
            );

            PlayCoreGlobalState::publish(
                &mut cx_async,
                PlayCoreStateEvent::TogglePlay(
                    player_window_id,
                    player_entity_id,
                    PlayStatic {
                        id: episode.id,
                        title: format!("{} - {}", episode.name, episode.source),
                        url: source,
                        headers: episode.headers,
                    },
                ),
            );
        })
        .detach();
    }
}
