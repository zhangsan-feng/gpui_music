mod music_player;

use crate::component::color::rgb_to_u32;
use crate::drive::NetworkStatic;
use crate::plugins::extractor::audio;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::scroll::{Scrollbar, ScrollbarAxis, ScrollbarMode};
use gpui_kit::component::{VirtualListScrollHandle, h_flex, v_flex, v_virtual_list};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use log::info;
use music_player::MusicPlayer;
use player_core::{PlayCoreGlobalState, PlayCoreStateEvent};
use std::rc::Rc;

#[derive(Clone)]
pub struct MusicPage {
    music_data: Vec<NetworkStatic>,
    is_loading: bool,
    vm_scroll_handle: VirtualListScrollHandle,
    music_search_keyword: Entity<InputState>,
    music_player: Entity<MusicPlayer>,
}

impl MusicPage {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> MusicPage {
        let music_player = cx.new(|cx| MusicPlayer::new(window, cx));
        cx.observe(&music_player, |_, _, cx| cx.notify()).detach();
        let play_core_id = music_player.read(cx)._play_core_id();
        let music_player_for_event = music_player.clone();
        PlayCoreGlobalState::subscribe(cx, move |this, event, cx| {
            let PlayCoreStateEvent::PlayBackFished(
                event_window_id,
                event_entity_id,
                finished_player,
            ) = event
            else {
                return;
            };

            if *event_entity_id != play_core_id {
                return;
            }

            let _ = finished_player;
            let _ = this
                .music_player
                .update(cx, |player, cx| player._play_next(*event_window_id, cx));
        })
        .detach();
        let mut s = MusicPage {
            music_data: Vec::new(),
            is_loading: false,
            vm_scroll_handle: VirtualListScrollHandle::new(),
            music_search_keyword: cx
                .new(|cx| InputState::new(window, cx).placeholder("input search music")),
            music_player: music_player_for_event,
        };
        s.init_data(cx);
        s
    }

    pub fn init_data(&mut self, cx: &mut Context<Self>) {
        let entity = cx.entity().clone();
        let mut cx_async = cx.to_async().clone();

        self.is_loading = true;

        cx.spawn(|_, _: &mut AsyncApp| async move {
            let res = tokio::spawn(async move { audio::recommend().await });

            match res.await {
                Ok(r) => {
                    entity.update(&mut cx_async, |this, cx| {
                        this.is_loading = false;
                        this.music_data = r.clone();
                        this.music_player
                            .update(cx, |player, cx| player._set_play_list(r, cx));

                        cx.notify()
                    });
                }
                Err(e) => info!("tokio runtime error: {:?}", e),
            }
        })
        .detach();
    }

    fn vm_btn_play_music(
        &self,
        data: NetworkStatic,
        index: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_current = { self.music_player.read(cx)._is_current_item(&data.id, cx) };
        Button::new(("music-play-index-", index))
            .label(if is_current { "播放中" } else { "播放" })
            .compact()
            .when(is_current, |button| button.primary())
            .when(!is_current, |button| button.ghost())
            .on_click(cx.listener(move |this, _, window, cx| {
                let _ = this.music_player.update(cx, |player, cx| {
                    player._play_item(index, window.window_handle().window_id(), cx)
                });
            }))
    }

    fn vm_list(&self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        const MUSIC_ROW_HEIGHT: f32 = 72.;

        v_virtual_list(
            cx.entity().clone(),
            "recommend-music-vm-list",
            Rc::new(
                self.music_data
                    .iter()
                    .map(|_| size(px(100.), px(MUSIC_ROW_HEIGHT)))
                    .collect(),
            ),
            |view, visible_range, _, cx| {
                visible_range
                    .map(|index| {
                        let data = view.music_data[index].clone();
                        let is_current =
                            { view.music_player.read(cx)._is_current_item(&data.id, cx) };
                        let row_bg = if is_current {
                            rgb_to_u32(239, 246, 255)
                        } else {
                            rgb_to_u32(255, 255, 255)
                        };
                        let row_border = if is_current {
                            rgb_to_u32(147, 197, 253)
                        } else {
                            rgb_to_u32(226, 232, 240)
                        };
                        let title = if data.name.trim().is_empty() {
                            "未命名歌曲".to_string()
                        } else {
                            data.name.clone()
                        };
                        let author = if data.author.trim().is_empty() {
                            "未知艺术家".to_string()
                        } else {
                            data.author.clone()
                        };

                        div().w_full().h(px(MUSIC_ROW_HEIGHT)).px_1().py_1().child(
                            h_flex()
                                .id(("music-row-", index))
                                .w_full()
                                .h_full()
                                .gap_3()
                                .px_3()
                                .rounded_xl()
                                .border_1()
                                .border_color(row_border)
                                .bg(row_bg)
                                .hover(|style| {
                                    style
                                        .bg(rgb_to_u32(248, 250, 252))
                                        .border_color(rgb_to_u32(191, 219, 254))
                                })
                                .child(
                                    div()
                                        .size(px(44.))
                                        .flex_shrink_0()
                                        .rounded_lg()
                                        .overflow_hidden()
                                        .bg(rgb_to_u32(241, 245, 249))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(if data.img.trim().is_empty() {
                                            div()
                                                .text_size(px(18.))
                                                .text_color(rgb_to_u32(148, 163, 184))
                                                .child("♪")
                                                .into_any_element()
                                        } else {
                                            img(data.img.clone())
                                                .size_full()
                                                .object_fit(ObjectFit::Cover)
                                                .into_any_element()
                                        }),
                                )
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .min_w_0()
                                        .gap_1()
                                        .child(
                                            div()
                                                .w_full()
                                                .text_size(px(14.))
                                                .font_weight(if is_current {
                                                    FontWeight::SEMIBOLD
                                                } else {
                                                    FontWeight::MEDIUM
                                                })
                                                .text_color(rgb_to_u32(15, 23, 42))
                                                .text_ellipsis()
                                                .child(title),
                                        )
                                        .child(
                                            div()
                                                .w_full()
                                                .text_size(px(12.))
                                                .text_color(rgb_to_u32(100, 116, 139))
                                                .text_ellipsis()
                                                .child(author),
                                        ),
                                )
                                .child(view.vm_btn_play_music(data, index, cx)),
                        )
                    })
                    .collect()
            },
        )
        .track_scroll(&self.vm_scroll_handle)
    }
}

impl Render for MusicPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .gap_3()
            .p_3()
            .bg(rgb_to_u32(255, 255, 255))
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .gap_3()
                    .p_3()
                    .border_color(rgb(0xEEE8F4))
                    .border_1()
                    .rounded_xl()
                    .shadow_sm()
                    .bg(rgb_to_u32(252, 249, 254))
                    .child(Input::new(&self.music_search_keyword))
                    .child(
                        h_flex().size_full().child(self.vm_list(window, cx)).child(
                            div().w(px(16.)).h_full().child(
                                Scrollbar::vertical(&self.vm_scroll_handle)
                                    .mode(ScrollbarMode::Always)
                                    .axis(ScrollbarAxis::Vertical)
                                    .viewport_from_layout(),
                            ),
                        ),
                    ),
            )
            .child(self.music_player.clone())
    }
}
