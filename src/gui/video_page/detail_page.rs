use super::VideoPage;
use crate::component::color::rgb_to_u32;
use crate::drive::NetworkStatic;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{IconName, h_flex, v_flex, v_virtual_list};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use regex::Regex;
use std::rc::Rc;
use std::sync::OnceLock;

const EPISODE_TILE_MIN_WIDTH: f32 = 84.;
const EPISODE_TILE_HEIGHT: f32 = 64.;
const EPISODE_ROW_HEIGHT: f32 = 80.;
const EPISODE_GAP: f32 = 12.;
const DETAIL_EPISODE_WIDTH_RESERVE: f32 = 696.;

fn episode_number(name: &str) -> Option<usize> {
    static EPISODE_PATTERN: OnceLock<Regex> = OnceLock::new();
    let captures = EPISODE_PATTERN
        .get_or_init(|| {
            Regex::new(
                r"(?i)(?:第\s*)?(\d{1,4})\s*(?:集|期|话|回)|(?:ep(?:isode)?|e)\s*0*(\d{1,4})",
            )
            .expect("episode number regex should be valid")
        })
        .captures(name)?;
    captures
        .get(1)
        .or_else(|| captures.get(2))
        .and_then(|value| value.as_str().parse().ok())
}

pub(super) fn sort_episodes(episodes: &mut [NetworkStatic]) {
    episodes.sort_by_key(|episode| episode_number(&episode.name).unwrap_or(usize::MAX));
}

fn episode_label(name: &str, index: usize) -> String {
    episode_number(name)
        .map(|number| format!("第{number}集"))
        .unwrap_or_else(|| format!("第{}集", index + 1))
}

impl VideoPage {
    pub(super) fn render_detail_page(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(source) = self.detail_source.as_ref() else {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    v_flex().items_center().gap_2().child(IconName::Info).child(
                        div()
                            .text_size(px(14.))
                            .text_color(rgb_to_u32(100, 116, 139))
                            .child("暂无详情"),
                    ),
                )
                .into_any_element();
        };

        if self.is_detail_loading {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    v_flex()
                        .items_center()
                        .gap_2()
                        .child(IconName::LoaderCircle)
                        .child(
                            div()
                                .text_size(px(14.))
                                .text_color(rgb_to_u32(100, 116, 139))
                                .child("详情加载中"),
                        ),
                )
                .into_any_element();
        }

        let cover = if source.img.trim().is_empty() {
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .gap_2()
                .text_size(px(13.))
                .text_color(rgb_to_u32(100, 116, 139))
                .child(IconName::GalleryVerticalEnd)
                .child("暂无封面")
                .into_any_element()
        } else {
            img(source.img.clone())
                .size_full()
                .object_fit(ObjectFit::Cover)
                .into_any_element()
        };

        let episodes = Rc::new(self.detail_result.clone());
        let page = cx.entity().clone();
        let episode_count = episodes.len();
        let title = if source.name.trim().is_empty() {
            "未命名视频"
        } else {
            &source.name
        };
        let source_name = if source.author.trim().is_empty() {
            source.category.clone()
        } else {
            source.author.clone()
        };
        let source_url = if source.source.trim().is_empty() {
            "暂无资源地址".to_string()
        } else {
            source.source.clone()
        };
        let episode_area_width = (window.bounds().size.width.as_f32()
            - DETAIL_EPISODE_WIDTH_RESERVE)
            .max(EPISODE_TILE_MIN_WIDTH);
        let episodes_per_row = ((episode_area_width + EPISODE_GAP)
            / (EPISODE_TILE_MIN_WIDTH + EPISODE_GAP))
            .floor()
            .max(1.) as usize;
        let row_count = if episode_count == 0 {
            0
        } else {
            (episode_count + episodes_per_row - 1) / episodes_per_row
        };
        let episode_sizes = Rc::new(
            (0..row_count)
                .map(|_| size(px(episode_area_width), px(EPISODE_ROW_HEIGHT)))
                .collect(),
        );
        let episode_list = v_virtual_list(
            cx.entity().clone(),
            "video-detail-episodes",
            episode_sizes,
            move |_, visible_range, _, _| {
                visible_range
                    .map(|row_index| {
                        let start = row_index * episodes_per_row;
                        let episode_tiles = episodes
                            .iter()
                            .skip(start)
                            .take(episodes_per_row)
                            .cloned()
                            .enumerate()
                            .map(|(offset, episode)| {
                                let index = start + offset;
                                let label = episode_label(&episode.name, index);
                                let page = page.clone();

                                Button::new(("video-episode-", index))
                                    .label(label)
                                    .outline()
                                    .rounded(px(10.))
                                    .flex_1()
                                    .min_w_0()
                                    .h(px(EPISODE_TILE_HEIGHT))
                                    .on_click(move |_, window, app| {
                                        let _ = page.update(app, |this, cx| {
                                            this.play_episode(episode.clone(), window, cx);
                                        });
                                    })
                            });

                        div().w_full().h(px(EPISODE_ROW_HEIGHT)).pb_3().child(
                            h_flex()
                                .w_full()
                                .h(px(EPISODE_TILE_HEIGHT))
                                .gap_3()
                                .items_center()
                                .children(episode_tiles),
                        )
                    })
                    .collect()
            },
        )
        .track_scroll(&self.detail_scroll_handler);

        v_flex()
            .size_full()
            .gap_3()
            .child(
                h_flex()
                    .w_full()
                    .h(px(44.))
                    .flex_shrink_0()
                    .items_center()
                    .gap_3()
                    .child(
                        Button::new("video-detail-back")
                            .icon(IconName::ArrowLeft)
                            .label("返回")
                            .ghost()
                            .compact()
                            .on_click(cx.listener(|this, _, _, cx| this.back_from_detail(cx))),
                    )
                    .child(div().w(px(1.)).h(px(20.)).bg(rgb_to_u32(226, 232, 240)))
                    .child(
                        div()
                            .text_size(px(14.))
                            .text_color(rgb_to_u32(100, 116, 139))
                            .child("视频详情"),
                    ),
            )
            .child(
                h_flex()
                    .w_full()
                    .flex_1()
                    .min_h_0()
                    .items_start()
                    .gap_4()
                    .child(
                        v_flex()
                            .w(px(344.))
                            .flex_shrink_0()
                            .rounded_xl()
                            .overflow_hidden()
                            .border_1()
                            .border_color(rgb_to_u32(231, 220, 235))
                            .bg(rgb_to_u32(255, 255, 255))
                            .shadow_sm()
                            .child(
                                div()
                                    .w_full()
                                    .h(px(240.))
                                    .p_3()
                                    .bg(rgb_to_u32(241, 245, 249))
                                    .child(
                                        div()
                                            .size_full()
                                            .rounded_lg()
                                            .overflow_hidden()
                                            .child(cover),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .w_full()
                                    .gap_3()
                                    .p_4()
                                    .child(
                                        div()
                                            .w_full()
                                            .text_size(px(18.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(rgb_to_u32(15, 23, 42))
                                            .text_ellipsis()
                                            .child(title.to_string()),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(
                                                div()
                                                    .rounded_full()
                                                    .px_2()
                                                    .py_1()
                                                    .bg(rgb_to_u32(239, 246, 255))
                                                    .text_size(px(12.))
                                                    .text_color(rgb_to_u32(37, 99, 235))
                                                    .child(format!("共 {episode_count} 集")),
                                            )
                                            .when(!source.category.trim().is_empty(), |this| {
                                                this.child(
                                                    div()
                                                        .rounded_full()
                                                        .px_2()
                                                        .py_1()
                                                        .bg(rgb_to_u32(250, 245, 255))
                                                        .text_size(px(12.))
                                                        .text_color(rgb_to_u32(126, 34, 206))
                                                        .child(source.category.clone()),
                                                )
                                            }),
                                    )
                                    .child(
                                        h_flex()
                                            .items_start()
                                            .gap_2()
                                            .text_size(px(13.))
                                            .child(IconName::CircleUser)
                                            .child(
                                                v_flex()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .gap_1()
                                                    .child(
                                                        div()
                                                            .text_size(px(11.))
                                                            .text_color(rgb_to_u32(148, 163, 184))
                                                            .child("来源"),
                                                    )
                                                    .child(
                                                        div()
                                                            .w_full()
                                                            .text_color(rgb_to_u32(51, 65, 85))
                                                            .text_ellipsis()
                                                            .child(source_name),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .w_full()
                                            .p_3()
                                            .rounded_lg()
                                            .bg(rgb_to_u32(248, 250, 252))
                                            .child(
                                                h_flex()
                                                    .items_start()
                                                    .gap_2()
                                                    .child(IconName::ExternalLink)
                                                    .child(
                                                        div()
                                                            .flex_1()
                                                            .min_w_0()
                                                            .text_size(px(11.))
                                                            .text_color(rgb_to_u32(100, 116, 139))
                                                            .text_ellipsis()
                                                            .child(source_url),
                                                    ),
                                            ),
                                    ),
                            ),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .min_h_0()
                            .gap_4()
                            .p_4()
                            .rounded_xl()
                            .border_1()
                            .border_color(rgb_to_u32(231, 220, 235))
                            .bg(rgb_to_u32(255, 255, 255))
                            .shadow_sm()
                            .child(
                                h_flex()
                                    .w_full()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        h_flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .w(px(32.))
                                                    .h(px(32.))
                                                    .rounded_lg()
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .bg(rgb_to_u32(239, 246, 255))
                                                    .text_color(rgb_to_u32(37, 99, 235))
                                                    .child(IconName::BookOpen),
                                            )
                                            .child(
                                                v_flex()
                                                    .gap_1()
                                                    .child(
                                                        div()
                                                            .text_size(px(16.))
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .text_color(rgb_to_u32(15, 23, 42))
                                                            .child("选集"),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(px(12.))
                                                            .text_color(rgb_to_u32(148, 163, 184))
                                                            .child("选择要播放的集数"),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .rounded_full()
                                            .px_3()
                                            .py_1()
                                            .bg(rgb_to_u32(248, 250, 252))
                                            .text_size(px(12.))
                                            .text_color(rgb_to_u32(100, 116, 139))
                                            .child(format!("{episode_count} 集")),
                                    ),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .h(px(1.))
                                    .flex_shrink_0()
                                    .bg(rgb_to_u32(241, 245, 249)),
                            )
                            .child(
                                h_flex()
                                    .flex_1()
                                    .min_h_0()
                                    .items_stretch()
                                    .gap_3()
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .h_full()
                                            .min_h_0()
                                            .px_2()
                                            .child(episode_list),
                                    )
                                    .child(Self::render_scrollbar(&self.detail_scroll_handler)),
                            ),
                    ),
            )
            .into_any_element()
    }
}
