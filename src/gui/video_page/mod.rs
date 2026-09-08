mod core;
mod detail_page;
mod recommend_page;
mod search_page;
mod ui;

use gpui_kit::component::input::InputState;
use gpui_kit::component::{VirtualListScrollHandle, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use player_core::{PlayCoreGlobalState, PlayCoreStateEvent};

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Page {
    Recommend,
    Search,
    Detail,
}

#[derive(Clone, Copy)]
pub(super) enum DetailOrigin {
    Recommend,
    Search,
}

pub struct VideoPage {
    current_page: Page,
    detail_origin: DetailOrigin,
    is_loading: bool,
    is_searching: bool,
    is_detail_loading: bool,
    search_keyword: Entity<InputState>,
    recommend_result: Vec<crate::drive::NetworkStatic>,
    search_result: std::collections::HashMap<String, Vec<crate::drive::NetworkStatic>>,
    detail_source: Option<crate::drive::NetworkStatic>,
    detail_result: Vec<crate::drive::NetworkStatic>,
    active_player_target: Option<(WindowId, EntityId)>,
    vm_scroll_handler: VirtualListScrollHandle,
    detail_scroll_handler: VirtualListScrollHandle,
}

impl VideoPage {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> VideoPage {
        let mut page = VideoPage {
            current_page: Page::Recommend,
            detail_origin: DetailOrigin::Recommend,
            is_loading: false,
            is_searching: false,
            is_detail_loading: false,
            search_keyword: cx.new(|cx| InputState::new(window, cx)),
            recommend_result: Vec::new(),
            search_result: std::collections::HashMap::new(),
            detail_source: None,
            detail_result: Vec::new(),
            active_player_target: None,
            vm_scroll_handler: VirtualListScrollHandle::new(),
            detail_scroll_handler: VirtualListScrollHandle::new(),
        };
        PlayCoreGlobalState::subscribe(cx, |this, event, cx| {
            let PlayCoreStateEvent::PlayBackFished(
                event_window_id,
                event_entity_id,
                finished_player,
            ) = event
            else {
                return;
            };

            let Some((active_window_id, active_entity_id)) = this.active_player_target else {
                return;
            };
            if active_window_id.as_u64() != event_window_id.as_u64()
                || active_entity_id != *event_entity_id
            {
                return;
            }

            let Some(current_index) = this
                .detail_result
                .iter()
                .position(|item| item.id == finished_player.id)
            else {
                return;
            };
            let Some(next) = this.detail_result.get(current_index + 1).cloned() else {
                return;
            };

            log::info!(
                "[video:auto-next] finished_episode_id={} next_episode_id={} window_id={} entity_id={}",
                finished_player.id,
                next.id,
                event_window_id.as_u64(),
                event_entity_id
            );
            this.request_play_episode(next, *event_window_id, *event_entity_id, cx);
        })
        .detach();
        page.init_data(cx);
        page
    }
}

impl Render for VideoPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match self.current_page {
            Page::Recommend => self.render_recommend_page(window, cx),
            Page::Search => self.render_search_page(window, cx),
            Page::Detail => self.render_detail_page(window, cx),
        };

        v_flex()
            .size_full()
            .gap_3()
            .p_3()
            .bg(rgb_to_u32(255, 255, 255))
            .when(self.current_page != Page::Detail, |this| {
                this.child(self.render_header(window, cx))
            })
            .child(div().flex_1().min_h_0().child(content))
            .into_any_element()
    }
}

fn rgb_to_u32(r: u8, g: u8, b: u8) -> Rgba {
    rgb((r as u32) << 16 | (g as u32) << 8 | b as u32)
}
