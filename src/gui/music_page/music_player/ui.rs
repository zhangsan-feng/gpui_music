use super::MusicPlayer;
use crate::component::color::rgb_to_u32;
use gpui_kit::component::{IconName, h_flex, v_flex};
use gpui_kit::*;

impl MusicPlayer {
    fn _render_control_button(
        &self,
        id: impl Into<ElementId>,
        label: impl IntoElement,
        is_primary: bool,
        click: impl Fn(&mut MusicPlayer, &ClickEvent, &mut Window, &mut Context<MusicPlayer>) + 'static,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(id)
            .h(px(34.))
            .px_3()
            .rounded_lg()
            .border_1()
            .border_color(if is_primary {
                rgb_to_u32(37, 99, 235)
            } else {
                rgb_to_u32(226, 232, 240)
            })
            .bg(if is_primary {
                rgb_to_u32(37, 99, 235)
            } else {
                rgb_to_u32(255, 255, 255)
            })
            .text_color(if is_primary {
                rgb_to_u32(255, 255, 255)
            } else {
                rgb_to_u32(51, 65, 85)
            })
            .text_size(px(12.))
            .font_weight(FontWeight::MEDIUM)
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(move |style| {
                if is_primary {
                    style.bg(rgb_to_u32(29, 78, 216))
                } else {
                    style.bg(rgb_to_u32(248, 250, 252))
                }
            })
            .on_click(cx.listener(click))
            .child(label)
    }

    fn _render_current_music(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let view_state = self.play_core.read(cx)._view_state();
        let current = self
            .current_index
            .and_then(|index| self.play_list.get(index));
        let title = view_state.player.title.clone().trim().to_string();
        let title = (!title.is_empty())
            .then_some(title)
            .unwrap_or_else(|| "未播放音乐".to_string());
        let author = current
            .map(|item| item.author.clone())
            .filter(|author| !author.trim().is_empty())
            .unwrap_or_else(|| "请选择一首音乐".to_string());
        let is_playing = view_state.is_playing;

        v_flex()
            .flex_1()
            .min_w_0()
            .gap_1()
            .child(
                div()
                    .w_full()
                    .text_size(px(14.))
                    .font_weight(if is_playing {
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
            )
    }
}

impl Render for MusicPlayer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let progress = self.play_core.update(cx, |player, cx| {
            player._progress_ui(window, cx).into_any_element()
        });
        let volume = self
            .play_core
            .update(cx, |player, cx| player._volume_ui(cx).into_any_element());
        let duration = self
            .play_core
            .update(cx, |player, _| player._duration_ui().into_any_element());
        let is_playing = self.play_core.read(cx)._view_state().is_playing;

        v_flex()
            .w_full()
            .gap_3()
            .p_3()
            .rounded_xl()
            .border_1()
            .border_color(rgb_to_u32(226, 232, 240))
            .bg(rgb_to_u32(252, 249, 254))
            .shadow_sm()
            .child(progress)
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_3()
                    .child(self._render_current_music(cx))
                    .child(
                        h_flex()
                            .flex_shrink_0()
                            .gap_2()
                            .items_center()
                            .child(self._render_control_button(
                                "music-player-previous",
                                IconName::ChevronLeft,
                                false,
                                move |this, _, window, cx| {
                                    this._play_previous(window.window_handle().window_id(), cx);
                                },
                                cx,
                            ))
                            .child(self._render_control_button(
                                "music-player-toggle",
                                if is_playing {
                                    IconName::Pause
                                } else {
                                    IconName::Play
                                },
                                true,
                                |this, _, window, cx| {
                                    this._toggle_play(window.window_handle().window_id(), cx);
                                },
                                cx,
                            ))
                            .child(self._render_control_button(
                                "music-player-next",
                                IconName::ChevronRight,
                                false,
                                move |this, _, window, cx| {
                                    this._play_next(window.window_handle().window_id(), cx);
                                },
                                cx,
                            )),
                    )
                    .child(
                        h_flex()
                            .flex_shrink_0()
                            .gap_3()
                            .items_center()
                            .child(volume)
                            .child(duration),
                    ),
            )
    }
}
