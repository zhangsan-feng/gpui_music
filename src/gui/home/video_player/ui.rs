use super::VideoPlayer;
use crate::component::color::rgb_to_u32;
use gpui_kit::component::{IconName, h_flex, v_flex};
use gpui_kit::*;

impl VideoPlayer {
    pub(super) fn _render_control_button(
        &self,
        id: impl Into<ElementId>,
        label: impl IntoElement,
        is_primary: bool,
        click: impl Fn(&mut VideoPlayer, &ClickEvent, &mut Window, &mut Context<VideoPlayer>) + 'static,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(id)
            .h(px(36.))
            .px_3()
            .rounded_lg()
            .border_1()
            .border_color(if is_primary {
                rgb_to_u32(190, 48, 139)
            } else {
                rgb_to_u32(218, 208, 225)
            })
            .bg(if is_primary {
                rgb_to_u32(190, 48, 139)
            } else {
                rgb_to_u32(255, 255, 255)
            })
            .text_color(if is_primary {
                rgb_to_u32(255, 255, 255)
            } else {
                rgb_to_u32(73, 66, 92)
            })
            .text_size(px(13.))
            .font_weight(FontWeight::MEDIUM)
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(move |style| {
                if is_primary {
                    style.bg(rgb_to_u32(165, 37, 120))
                } else {
                    style.bg(rgb_to_u32(248, 243, 249))
                }
            })
            .on_click(cx.listener(click))
            .child(label)
    }

    fn _render_player_content(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let frame = self.play_core.update(cx, |player, cx| {
            player._frame_ui(window, cx).into_any_element()
        });
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
            .flex_1()
            .h_full()
            .min_w_0()
            .min_h_0()
            .gap_4()
            .p_4()
            .rounded_xl()
            .border_1()
            .border_color(rgb_to_u32(231, 220, 235))
            .bg(rgb_to_u32(255, 255, 255))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .rounded_xl()
                    .overflow_hidden()
                    .border_1()
                    .border_color(rgb_to_u32(218, 208, 225))
                    .shadow_sm()
                    .child(frame),
            )
            .child(
                v_flex()
                    .mt_auto()
                    .w_full()
                    .gap_3()
                    .p_3()
                    .rounded_xl()
                    .border_1()
                    .border_color(rgb_to_u32(218, 208, 225))
                    .bg(rgb_to_u32(252, 249, 254))
                    .shadow_sm()
                    .child(progress)
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .justify_between()
                            .child(
                                h_flex()
                                    .flex_shrink_0()
                                    .gap_2()
                                    .items_center()
                                    .child(self._render_control_button(
                                        "custmer-player-settings",
                                        IconName::Menu,
                                        false,
                                        |this, _, _, cx| this.toggle_side_panel(cx),
                                        cx,
                                    ))
                                    .child(self._render_control_button(
                                        "custmer-player-previous",
                                        IconName::ChevronLeft,
                                        false,
                                        |this, _, _, cx| this._play_previous(cx),
                                        cx,
                                    ))
                                    .child(self._render_control_button(
                                        "custmer-player-toggle",
                                        if is_playing {
                                            IconName::Pause
                                        } else {
                                            IconName::Play
                                        },
                                        true,
                                        |this, _, _, cx| {
                                            let _ = this
                                                .play_core
                                                .update(cx, |player, cx| player._toggle_play(cx));
                                        },
                                        cx,
                                    ))
                                    .child(self._render_control_button(
                                        "custmer-player-next",
                                        IconName::ChevronRight,
                                        false,
                                        |this, _, _, cx| this._play_next(cx),
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
                    ),
            )
    }
}

impl Render for VideoPlayer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .min_w_0()
            .min_h_0()
            .p_3()
            .overflow_hidden()
            .rounded_xl()
            .bg(rgb_to_u32(242, 237, 246))
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _window, cx| {
                let item = this.play_core.read(cx)._file_drop_source(paths);
                if let Some(item) = item {
                    this._append_and_play(item, cx);
                }
            }))
            .child(
                h_flex()
                    .size_full()
                    .min_w_0()
                    .min_h_0()
                    .items_stretch()
                    .gap_4()
                    .overflow_hidden()
                    .child(self._render_player_content(window, cx))
                    .child(self._render_media_queue(cx)),
            )
    }
}
