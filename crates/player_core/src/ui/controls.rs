use crate::core::{ProgressDrag, VolumeDrag};
use crate::{PlatState, PlayCore, rgb_to_u32};
use gpui_kit::component::{ElementExt, IconName, h_flex, v_flex};
use gpui_kit::*;
use std::time::Duration;

impl PlayCore {
    pub fn render_control(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let total = self.progress.duration.unwrap_or(Duration::ZERO);
        let display_position = self.display_position();

        v_flex()
            .w_full()
            .p_2()
            .gap_2()
            .rounded_xl()
            .border_1()
            .border_color(rgb_to_u32(203, 213, 225))
            .bg(rgb_to_u32(248, 250, 252))
            .child(self.player_progress_control_ui(window, cx))
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .child(self.player_control_ui(cx))
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(self.player_volume_control_ui(cx))
                            .child(self.player_duration_display_ui(display_position, total)),
                    ),
            )
            .with_animations(
                "video-player-animations",
                vec![Animation::new(Duration::from_millis(500)).with_easing(ease_in_out)],
                move |el, _, delta| el.h(px(75.) * delta).opacity(0.2 + 0.8 * delta),
            )
    }

    pub(crate) fn player_volume_control_ui(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let volume_ratio = self.volume.value;
        let volume_bar_width = 150.0;

        h_flex().child(
            h_flex()
                .w(px(220.))
                .gap_2()
                .items_center()
                .child(
                    div()
                        .size(px(32.))
                        .rounded_full()
                        .bg(rgb_to_u32(239, 246, 255))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(16.))
                        .child("🔊"),
                )
                .child(
                    div()
                        .w(px(35.))
                        .text_size(px(11.))
                        .text_color(rgb_to_u32(100, 116, 139))
                        .child(format!("{:.0}%", volume_ratio * 100.0)),
                )
                .child(
                    div()
                        .h(px(7.))
                        .w(px(volume_bar_width))
                        .rounded_full()
                        .bg(rgb_to_u32(226, 232, 240))
                        .cursor_pointer()
                        .on_prepaint({
                            let volume_bar_entity = cx.entity();
                            move |bounds: Bounds<Pixels>, _: &mut Window, cx: &mut App| {
                                let _ = volume_bar_entity.update(cx, |this, _| {
                                    this.volume.bar_bounds = Some(bounds);
                                });
                            }
                        })
                        .id("video_volume_bar")
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, event: &MouseDownEvent, _, cx| {
                                if let Some(bounds) = this.volume.bar_bounds {
                                    let _ = this._drag_volume_at(event.position, bounds, cx);
                                }
                            }),
                        )
                        .on_drag(VolumeDrag, |_value, _offset, _, cx| cx.new(|_| Empty))
                        .on_drag_move::<VolumeDrag>(cx.listener(
                            |this, event: &DragMoveEvent<VolumeDrag>, _, cx| {
                                let _ =
                                    this._drag_volume_at(event.event.position, event.bounds, cx);
                            },
                        ))
                        .child(
                            div()
                                .h(px(7.))
                                .w(px(volume_bar_width * volume_ratio))
                                .rounded_full()
                                .bg(rgb_to_u32(59, 130, 246)),
                        ),
                ),
        )
    }

    pub(crate) fn player_duration_display_ui(
        &self,
        position: Duration,
        total: Duration,
    ) -> impl IntoElement {
        h_flex()
            .gap_2()
            .items_center()
            .child(self.format_time(position))
            .child("/")
            .child(self.format_time(total))
    }

    pub(crate) fn player_progress_control_ui(
        &self,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let progress_ratio = self.progress_ratio();
        let progress_bar_width = self
            .progress
            .bar_bounds
            .as_ref()
            .map(|bounds| bounds.size.width.as_f32())
            .unwrap_or(0.0);
        let progress_bar_entity = cx.entity();

        v_flex().gap_2().child(
            div()
                .h(px(7.))
                .w_full()
                .rounded_full()
                .bg(rgb_to_u32(226, 232, 240))
                .cursor_pointer()
                .on_prepaint({
                    let progress_bar_entity = progress_bar_entity.clone();
                    move |bounds: Bounds<Pixels>, _: &mut Window, cx: &mut App| {
                        let _ = progress_bar_entity.update(cx, |this, _| {
                            this.progress.bar_bounds = Some(bounds);
                        });
                    }
                })
                .id("video_progress_bar")
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, event: &MouseDownEvent, _, cx| {
                        if let Some(bounds) = this.progress.bar_bounds {
                            if let Some(target) = this.get_progress_position(event.position, bounds)
                            {
                                this.seek(target, cx);
                                this.progress.is_dragging = false;
                                this.progress.pending_seek_position = None;
                                cx.notify();
                            }
                        }
                    }),
                )
                .on_drag(ProgressDrag, |_, _, _, cx: &mut App| cx.new(|_| Empty))
                .on_drag_move::<ProgressDrag>(cx.listener(
                    |this, event: &DragMoveEvent<ProgressDrag>, _, cx| {
                        if let Some(target) =
                            this.get_progress_position(event.event.position, event.bounds)
                        {
                            let _ = this._drag_progress(target, cx);
                        }
                    },
                ))
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        if this.progress.is_dragging {
                            let _ = this._commit_progress_drag(cx);
                        }
                    }),
                )
                .on_mouse_up_out(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        if this.progress.is_dragging {
                            let _ = this._commit_progress_drag(cx);
                        }
                    }),
                )
                .child(
                    div()
                        .h(px(7.))
                        .w(px(progress_bar_width * progress_ratio))
                        .rounded_full()
                        .bg(rgb_to_u32(59, 130, 246)),
                ),
        )
    }

    fn render_control_button(
        &self,
        id: impl Into<ElementId>,
        label: impl IntoElement,
        enabled: bool,
        click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        let mut button = div()
            .size(px(36.))
            .rounded_md()
            .bg(rgb_to_u32(248, 250, 252))
            .border_1()
            .border_color(rgb_to_u32(226, 232, 240))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(13.))
            .text_color(rgb_to_u32(15, 23, 42))
            .id(id);
        if enabled {
            button = button
                .cursor_pointer()
                .hover(|style| {
                    style
                        .bg(rgb_to_u32(239, 246, 255))
                        .border_color(rgb_to_u32(147, 197, 253))
                        .text_color(rgb_to_u32(37, 99, 235))
                })
                .on_click(click);
        } else {
            button = button.opacity(0.45);
        }
        button.child(label)
    }

    pub(crate) fn player_control_ui(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let can_toggle_play = self.playback_controls_enabled();
        h_flex()
            .gap_2()
            .child(self.render_control_button(
                "video_play_button",
                if self.pipeline.state == PlatState::Playing {
                    IconName::Pause
                } else {
                    IconName::Play
                },
                can_toggle_play,
                cx.listener(|this, _, _, cx| {
                    this.toggle_play(cx);
                }),
            ))
            .child(self.render_control_button(
                "video_retry_button",
                IconName::Undo2,
                true,
                cx.listener(|this, _, _, cx| {
                    this.retry(cx);
                }),
            ))
    }
}
