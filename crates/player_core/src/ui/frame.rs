use crate::{PlatState, PlayCore, rgb_to_u32};
use gpui_kit::component::{ElementExt, IconName, text::markdown, v_flex};
use gpui_kit::*;

impl PlayCore {
    pub fn render_frame(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let frame_aspect = self.player_static_info.frame_info.aspect_ratio();
        let fitted_frame_size = self.frame.surface_bounds.map(|bounds| {
            let container_width = bounds.size.width.as_f32().max(1.0);
            let container_height = bounds.size.height.as_f32().max(1.0);
            let container_aspect = container_width / container_height;

            if container_aspect > frame_aspect {
                (container_height * frame_aspect, container_height)
            } else {
                (container_width, container_width / frame_aspect)
            }
        });
        let current_image = self.frame.images.current_image();
        let pause_overlay = match (&self.pipeline.state, current_image.is_some()) {
            (PlatState::Paused, true) => div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(0xFFFFFF))
                .text_size(px(48.))
                .opacity(0.72)
                .child(IconName::Play)
                .into_any_element(),
            _ => div().into_any_element(),
        };
        let error_overlay = match &self.pipeline.state {
            PlatState::Error(message) => v_flex()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(rgb_to_u32(15, 23, 42))
                .opacity(0.86)
                .child(
                    div()
                        .max_w(px(520.))
                        .px_4()
                        .py_3()
                        .rounded_lg()
                        .bg(rgb_to_u32(127, 29, 29))
                        .child(
                            markdown(message.clone())
                                .selectable(true)
                                .text_color(rgb(0xFECACA))
                                .cursor_text(),
                        ),
                )
                .into_any_element(),
            _ => div().into_any_element(),
        };

        div()
            .flex_grow_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .relative()
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.toggle_play(cx);
                }),
            )
            .justify_center()
            .items_center()
            .overflow_hidden()
            .rounded_xl()
            .bg(rgb_to_u32(15, 23, 42))
            .border_1()
            .border_color(rgb_to_u32(30, 41, 59))
            .on_prepaint({
                let player_entity = cx.entity();
                move |bounds: Bounds<Pixels>, _: &mut Window, cx: &mut App| {
                    let _ = player_entity.update(cx, |player, cx| {
                        let changed = player
                            .frame
                            .surface_bounds
                            .map(|current| {
                                current.size.width != bounds.size.width
                                    || current.size.height != bounds.size.height
                            })
                            .unwrap_or(true);
                        if changed {
                            player.frame.surface_bounds = Some(bounds);
                            cx.notify();
                        }
                    });
                }
            })
            .child(if let Some(frame) = current_image {
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .justify_center()
                    .items_center()
                    .child(if let Some((width, height)) = fitted_frame_size {
                        img(frame)
                            .w(px(width))
                            .h(px(height))
                            .object_fit(ObjectFit::Cover)
                            .into_any_element()
                    } else {
                        img(frame)
                            .size_full()
                            .object_fit(ObjectFit::Cover)
                            .into_any_element()
                    })
                    .into_any_element()
            } else {
                v_flex()
                    .absolute()
                    .inset_0()
                    .flex()
                    .justify_center()
                    .items_center()
                    .child(
                        div().px_4().child(
                            markdown(match &self.pipeline.state {
                                PlatState::Playing | PlatState::Paused => String::new(),
                                PlatState::Loading => "加载中".to_string(),
                                PlatState::UnLoading => "没有加载播放来源".to_string(),
                                PlatState::Error(_) => String::new(),
                            })
                            .selectable(true)
                            .text_color(rgb(0xCBD5E1))
                            .cursor_text(),
                        ),
                    )
                    .into_any_element()
            })
            .child(pause_overlay)
            .child(error_overlay)
    }
}
