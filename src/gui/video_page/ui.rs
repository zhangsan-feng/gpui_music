use super::{Page, VideoPage};
use crate::component::color::rgb_to_u32;
use crate::drive::NetworkStatic;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::Input;
use gpui_kit::component::scroll::{Scrollbar, ScrollbarAxis, ScrollbarMode};
use gpui_kit::component::{h_flex, v_flex, v_virtual_list};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use std::rc::Rc;

impl VideoPage {
    pub(super) fn render_header(&self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let result_count = match self.current_page {
            Page::Recommend => self.recommend_result.len(),
            Page::Search => self.search_result.values().map(Vec::len).sum(),
            Page::Detail => 0,
        };

        h_flex()
            .items_center()
            .h(px(64.))
            .w_full()
            .gap_3()
            .px_3()
            .rounded_xl()
            .border_1()
            .border_color(rgb_to_u32(238, 232, 244))
            .bg(rgb_to_u32(252, 249, 254))
            .child(
                div()
                    .flex_grow_1()
                    .child(Input::new(&self.search_keyword).cleanable(true)),
            )
            .child(
                Button::new("video-page-search-btn")
                    .label(if self.is_searching {
                        "搜索中"
                    } else {
                        "搜索"
                    })
                    .on_click(cx.listener(|this, _, window, cx| this.search_video(window, cx))),
            )
            .when(self.current_page == Page::Search, |this| {
                this.child(
                    Button::new("video-page-clear-search-btn")
                        .label("推荐")
                        .ghost()
                        .compact()
                        .on_click(cx.listener(|this, _, window, cx| this.clear_search(window, cx))),
                )
            })
            .child(
                div()
                    .rounded_full()
                    .bg(rgb_to_u32(239, 246, 255))
                    .text_size(px(12.))
                    .text_color(rgb_to_u32(37, 99, 235))
                    .child(result_count.to_string()),
            )
            .into_any_element()
    }

    pub(super) fn render_video_list(
        &self,
        items: Vec<NetworkStatic>,
        list_id: &'static str,
        empty_text: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        const VIDEO_LIST_ROW_HEIGHT: f32 = 124.;

        if items.is_empty() {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb_to_u32(100, 116, 139))
                .child(empty_text)
                .into_any_element();
        }

        let available_width = (window.bounds().size.width.as_f32() - 100.).max(240.);
        let items = Rc::new(items);

        let list = v_virtual_list(
            cx.entity().clone(),
            list_id,
            Rc::new(
                (0..items.len())
                    .map(|_| size(px(available_width), px(VIDEO_LIST_ROW_HEIGHT)))
                    .collect(),
            ),
            move |view, visible_range, _, cx| {
                visible_range
                    .map(|index| view.render_video_card(items[index].clone(), cx))
                    .collect()
            },
        )
        .track_scroll(&self.vm_scroll_handler);

        h_flex()
            .size_full()
            .min_h_0()
            .items_stretch()
            .gap_2()
            .child(div().flex_1().min_w_0().h_full().min_h_0().child(list))
            .child(Self::render_scrollbar(&self.vm_scroll_handler))
            .into_any_element()
    }

    fn render_video_card(&self, data: NetworkStatic, cx: &mut Context<Self>) -> AnyElement {
        const VIDEO_LIST_ROW_HEIGHT: f32 = 124.;

        let cover = if data.img.trim().is_empty() {
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(13.))
                .text_color(rgb_to_u32(100, 116, 139))
                .child("暂无封面")
                .into_any_element()
        } else {
            img(data.img.clone())
                .size_full()
                .object_fit(ObjectFit::Cover)
                .into_any_element()
        };
        let mut extra_fields = data.extra.iter().collect::<Vec<_>>();
        extra_fields.sort_by(|left, right| left.0.cmp(right.0));
        let extra_fields = extra_fields
            .into_iter()
            .map(|(key, value)| {
                let value = value
                    .as_str()
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| value.to_string());
                div()
                    .w_full()
                    .min_w_0()
                    .text_size(px(11.))
                    .text_color(rgb_to_u32(148, 163, 184))
                    .text_ellipsis()
                    .child(format!("{key}: {value}"))
            })
            .collect::<Vec<_>>();

        div()
            .w_full()
            .h(px(VIDEO_LIST_ROW_HEIGHT))
            .pb_3()
            .child(
                div()
                    .id(format!("video-card-{}", data.id))
                    .w_full()
                    .h(px(112.))
                    .rounded_lg()
                    .border_1()
                    .border_color(rgb_to_u32(226, 232, 240))
                    .bg(rgb_to_u32(255, 255, 255))
                    .overflow_hidden()
                    .cursor_pointer()
                    .hover(|style| {
                        style
                            .bg(rgb_to_u32(248, 250, 252))
                            .border_color(rgb_to_u32(148, 163, 184))
                    })
                    .on_click({
                        let data = data.clone();
                        cx.listener(move |this, _, window, cx| {
                            this.open_detail(data.clone(), window, cx);
                        })
                    })
                    .child(
                        h_flex()
                            .size_full()
                            .child(
                                div()
                                    .w(px(168.))
                                    .h_full()
                                    .flex_shrink_0()
                                    .overflow_hidden()
                                    .bg(rgb_to_u32(241, 245, 249))
                                    .child(cover),
                            )
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .h_full()
                                    .p_3()
                                    .gap_1()
                                    .items_start()
                                    .child(
                                        div()
                                            .w_full()
                                            .text_size(px(15.))
                                            .text_color(rgb_to_u32(15, 23, 42))
                                            .text_ellipsis()
                                            .child(data.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .w_full()
                                            .text_size(px(12.))
                                            .text_color(rgb_to_u32(100, 116, 139))
                                            .text_ellipsis()
                                            .child(format!(
                                                "来源：{}",
                                                if data.author.is_empty() {
                                                    data.category.clone()
                                                } else {
                                                    data.author.clone()
                                                }
                                            )),
                                    )
                                    .children(extra_fields),
                            ),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn render_scrollbar(
        handle: &gpui_kit::component::VirtualListScrollHandle,
    ) -> AnyElement {
        div()
            .w(px(16.))
            .h_full()
            .child(
                Scrollbar::vertical(handle)
                    .mode(ScrollbarMode::Always)
                    .axis(ScrollbarAxis::Vertical)
                    .viewport_from_layout(),
            )
            .into_any_element()
    }
}
