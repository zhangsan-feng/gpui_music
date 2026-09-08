use super::core::export_format_label;
use super::{SidePanelState, VideoPlayer};
use crate::component::color::rgb_to_u32;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::Input;
use gpui_kit::component::menu::{DropdownMenu, PopupMenu, PopupMenuItem};
use gpui_kit::component::scroll::{Scrollbar, ScrollbarAxis, ScrollbarMode};
use gpui_kit::component::{Disableable, IconName, h_flex, v_flex};
use gpui_kit::*;
use player_core::{PlayCoreMediaType, PlayCoreTranscodeFormat, PlayCoreViewState};
use std::rc::Rc;
use std::time::Duration;

impl VideoPlayer {
    pub(super) fn _render_media_queue(&self, cx: &mut Context<Self>) -> AnyElement {
        let playlist_content = self._render_playlist_content(cx);
        let project_info = self._render_project_info(cx);
        let media_queue = self._render_media_queue_content(playlist_content, project_info, cx);

        self._render_side_panel(media_queue)
    }

    fn _render_side_panel(&self, media_queue: AnyElement) -> AnyElement {
        let side_panel_state = self.side_panel_state;
        let animation_id = self.side_panel_animation_id;
        let panel_fraction = 1.0 / 3.0;
        let panel = div()
            .h_full()
            .w(relative(panel_fraction))
            .flex_shrink_0()
            .overflow_hidden()
            .child(
                v_flex()
                    .size_full()
                    .min_h_0()
                    .gap_3()
                    .p_3()
                    .rounded_xl()
                    .overflow_hidden()
                    .border_1()
                    .border_color(rgb_to_u32(231, 220, 235))
                    .bg(rgb_to_u32(252, 249, 254))
                    .child(media_queue),
            );

        match side_panel_state {
            SidePanelState::Open => panel.into_any_element(),
            SidePanelState::Closed => panel.w(relative(0.)).opacity(0.).into_any_element(),
            SidePanelState::Opening | SidePanelState::Closing => {
                let opening = side_panel_state == SidePanelState::Opening;
                panel
                    .with_animation(
                        format!("video-player-media-queue-{animation_id}"),
                        Animation::new(Duration::from_millis(500)).with_easing(ease_in_out),
                        move |el, delta| {
                            let progress = if opening { delta } else { 1.0 - delta };
                            el.w(relative(panel_fraction * progress)).opacity(progress)
                        },
                    )
                    .into_any_element()
            }
        }
    }

    fn _render_media_queue_content(
        &self,
        playlist_content: AnyElement,
        project_info: AnyElement,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        v_flex()
            .size_full()
            .min_h_0()
            .gap_3()
            .child(
                h_flex()
                    .h(px(40.))
                    .w_full()
                    .flex_shrink_0()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Input::new(&self.network_url_input).cleanable(true)),
                    )
                    .child(self._render_control_button(
                        "custmer-player-play-url",
                        "播放",
                        false,
                        |this, _, window, cx| this._play_network_url(window, cx),
                        cx,
                    )),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .gap_3()
                    .child(
                        v_flex()
                            .flex_1()
                            .min_h_0()
                            .gap_2()
                            .p_3()
                            .rounded_lg()
                            .overflow_hidden()
                            .border_1()
                            .border_color(rgb_to_u32(231, 220, 235))
                            .bg(rgb_to_u32(255, 255, 255))
                            .child(playlist_content),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_h_0()
                            .gap_3()
                            .p_3()
                            .rounded_lg()
                            .overflow_hidden()
                            .border_1()
                            .border_color(rgb_to_u32(231, 220, 235))
                            .bg(rgb_to_u32(255, 255, 255))
                            .child(project_info),
                    ),
            )
            .into_any_element()
    }

    fn _render_project_info(&self, cx: &mut Context<Self>) -> AnyElement {
        let view_state = self.play_core.read(cx)._view_state();
        let resource = if !view_state.player.title.trim().is_empty() {
            view_state.player.title.clone()
        } else if !view_state.player.id.trim().is_empty() {
            view_state.player.id.clone()
        } else if !view_state.player.url.trim().is_empty() {
            view_state.player.url.clone()
        } else {
            "暂无资源".to_string()
        };
        let can_export = !view_state.player.url.trim().is_empty()
            && matches!(
                view_state.media_type,
                PlayCoreMediaType::Audio | PlayCoreMediaType::Video
            );
        let export_controls = if can_export {
            v_flex()
                .mt_auto()
                .w_full()
                .gap_2()
                .child(self._render_export_status())
                .child(self._render_export_button(cx))
                .into_any_element()
        } else {
            div().into_any_element()
        };

        v_flex()
            .size_full()
            .gap_3()
            .child(
                h_flex()
                    .w_full()
                    .items_start()
                    .gap_3()
                    .child(
                        div()
                            .w(px(64.))
                            .flex_shrink_0()
                            .text_size(px(12.))
                            .text_color(rgb_to_u32(148, 140, 163))
                            .child("资源"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(12.))
                            .text_color(rgb_to_u32(73, 66, 92))
                            .text_ellipsis()
                            .child(resource),
                    ),
            )
            .child(self._render_media_info(&view_state))
            .child(export_controls)
            .into_any_element()
    }

    fn _render_media_info(&self, view_state: &PlayCoreViewState) -> AnyElement {
        let media_type = match view_state.media_type {
            PlayCoreMediaType::Audio => "音乐",
            PlayCoreMediaType::Video => "视频",
            PlayCoreMediaType::Unknown => "未知",
        };
        let resolution = if view_state.frame_width > 0.0 && view_state.frame_height > 0.0 {
            format!(
                "{} × {}",
                view_state.frame_width as u32, view_state.frame_height as u32
            )
        } else {
            "未知".to_string()
        };
        let frame_rate = if view_state.frame_rate > 0.0 {
            format!("{:.2} FPS", view_state.frame_rate)
        } else {
            "未知".to_string()
        };
        let codec = view_state.codec.as_deref().unwrap_or("未知").to_string();

        v_flex()
            .w_full()
            .gap_2()
            .child(self._render_project_info_row("媒体类型", media_type.to_string()))
            .child(self._render_project_info_row("分辨率", resolution))
            .child(self._render_project_info_row("帧率", frame_rate))
            .child(self._render_project_info_row("编码", codec))
            .into_any_element()
    }

    fn _render_project_info_row(&self, label: &str, value: String) -> AnyElement {
        h_flex()
            .w_full()
            .items_center()
            .gap_3()
            .child(
                div()
                    .w(px(64.))
                    .flex_shrink_0()
                    .text_size(px(12.))
                    .text_color(rgb_to_u32(148, 140, 163))
                    .child(label.to_string()),
            )
            .child(
                div()
                    .flex_1()
                    .text_size(px(12.))
                    .text_color(rgb_to_u32(73, 66, 92))
                    .text_ellipsis()
                    .child(value),
            )
            .into_any_element()
    }

    fn _render_export_status(&self) -> AnyElement {
        let Some(status) = &self.export_status else {
            return div().into_any_element();
        };
        let (message, color) = match status {
            super::ExportStatus::Working(message) => (message.as_str(), rgb_to_u32(148, 140, 163)),
            super::ExportStatus::Success(message) => (message.as_str(), rgb_to_u32(41, 139, 78)),
            super::ExportStatus::Error(message) => (message.as_str(), rgb_to_u32(190, 48, 90)),
        };

        div()
            .w_full()
            .text_size(px(12.))
            .text_color(color)
            .text_ellipsis()
            .child(message.to_owned())
            .into_any_element()
    }

    fn _render_export_button(&self, cx: &mut Context<Self>) -> AnyElement {
        let view_state = self.play_core.read(cx)._view_state();
        let has_source = !view_state.player.url.trim().is_empty();
        let media_type = view_state.media_type;
        if !has_source || media_type == PlayCoreMediaType::Unknown {
            return div().into_any_element();
        }
        let disabled = self.export_in_progress || !has_source;
        let export_label = match media_type {
            PlayCoreMediaType::Video => "导出视频",
            PlayCoreMediaType::Audio => "导出音乐",
            PlayCoreMediaType::Unknown => unreachable!("未知媒体"),
        };
        let player = cx.entity().clone();
        let add_item =
            move |menu: PopupMenu, format: PlayCoreTranscodeFormat, player: Entity<VideoPlayer>| {
                let label = export_format_label(format);
                menu.item(PopupMenuItem::new(label).disabled(disabled).on_click(
                    move |_, _, app| {
                        let _ = player.update(app, |this, cx| this.export_media(format, cx));
                    },
                ))
            };

        Button::new("custmer-player-export")
            .w_full()
            .label(if self.export_in_progress {
                "导出中..."
            } else {
                export_label
            })
            .icon(IconName::ArrowDown)
            .dropdown_caret(true)
            .secondary()
            .disabled(disabled)
            .dropdown_menu_with_anchor(Anchor::BottomRight, move |menu, _, _| match media_type {
                PlayCoreMediaType::Video => {
                    let menu = menu.item(PopupMenuItem::label("视频格式")).separator();
                    let menu = add_item(menu, PlayCoreTranscodeFormat::Mp4, player.clone());
                    let menu = add_item(menu, PlayCoreTranscodeFormat::Mkv, player.clone());
                    add_item(menu, PlayCoreTranscodeFormat::MOV, player.clone())
                }
                PlayCoreMediaType::Audio => {
                    let menu = menu.item(PopupMenuItem::label("音频格式")).separator();
                    let menu = add_item(menu, PlayCoreTranscodeFormat::Mp3, player.clone());
                    let menu = add_item(menu, PlayCoreTranscodeFormat::FLAC, player.clone());
                    let menu = add_item(menu, PlayCoreTranscodeFormat::WAV, player.clone());
                    add_item(menu, PlayCoreTranscodeFormat::AAC, player.clone())
                }
                PlayCoreMediaType::Unknown => unreachable!("未知媒体"),
            })
            .into_any_element()
    }

    fn _render_playlist_content(&self, cx: &mut Context<Self>) -> AnyElement {
        let play_list = Rc::new(self.play_list.clone());
        let current_index = self.current_index;
        let player = cx.entity().clone();
        let play_list_state = self.play_list_state.clone();

        if play_list.is_empty() {
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(13.))
                .text_color(rgb_to_u32(148, 140, 163))
                .child("暂无播放内容")
                .into_any_element()
        } else {
            let playlist_list = list(play_list_state.clone(), move |index, _, _| {
                let Some(item) = play_list.get(index).cloned() else {
                    return div().into_any_element();
                };
                let is_current = current_index == Some(index);
                let title = if !item.title.trim().is_empty() {
                    item.title
                } else if !item.id.trim().is_empty() {
                    item.id
                } else {
                    format!("播放项 {}", index + 1)
                };
                let player = player.clone();

                div()
                    .id(("custmer-player-play-list-item", index))
                    .w_full()
                    .min_h(px(44.))
                    .px_3()
                    .rounded_lg()
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .bg(if is_current {
                        rgb_to_u32(252, 226, 244)
                    } else {
                        rgb_to_u32(255, 255, 255)
                    })
                    .text_color(if is_current {
                        rgb_to_u32(168, 38, 122)
                    } else {
                        rgb_to_u32(73, 66, 92)
                    })
                    .hover(|style| style.bg(rgb_to_u32(247, 240, 249)))
                    .on_click(move |_, _, app| {
                        let _ = player.update(app, |this, cx| this._play_item(index, cx));
                    })
                    .child(
                        div()
                            .w_full()
                            .text_size(px(13.))
                            .text_ellipsis()
                            .child(title),
                    )
                    .into_any_element()
            })
            .size_full();

            h_flex()
                .size_full()
                .min_h_0()
                .items_stretch()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .min_h_0()
                        .child(playlist_list),
                )
                .child(
                    div().w(px(8.)).h_full().child(
                        Scrollbar::vertical(&play_list_state)
                            .mode(ScrollbarMode::Always)
                            .axis(ScrollbarAxis::Vertical),
                    ),
                )
                .into_any_element()
        }
    }
}
