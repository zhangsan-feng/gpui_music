mod core;
mod side_panel;
mod ui;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{AppContext, Context, Entity, ListAlignment, ListState, Subscription, Window};
use player_core::PlayCore;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SidePanelState {
    Open,
    Closed,
    Opening,
    Closing,
}

#[derive(Clone, Debug)]
enum ExportStatus {
    Working(String),
    Success(String),
    Error(String),
}

pub struct VideoPlayer {
    play_core: Entity<PlayCore>,
    play_list: Vec<player_core::PlayStatic>,
    current_index: Option<usize>,
    side_panel_state: SidePanelState,
    side_panel_animation_id: u64,
    play_list_state: ListState,
    network_url_input: Entity<InputState>,
    _network_url_input_subscription: Subscription,
    export_in_progress: bool,
    export_status: Option<ExportStatus>,
}

impl VideoPlayer {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let play_core = cx.new(|cx| PlayCore::new(window, cx));
        cx.observe(&play_core, |_, _, cx| cx.notify()).detach();
        let network_url_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("输入网络视频或音频链接，按回车播放")
        });
        let network_url_input_subscription = cx.subscribe_in(
            &network_url_input,
            window,
            |this, _input, event: &InputEvent, window, cx| {
                if !matches!(event, InputEvent::PressEnter { .. }) {
                    return;
                }

                this._play_network_url(window, cx);
            },
        );

        Self {
            play_core,
            play_list: Vec::new(),
            current_index: None,
            side_panel_state: SidePanelState::Open,
            side_panel_animation_id: 0,
            play_list_state: ListState::new(0, ListAlignment::Top, gpui_kit::px(64.))
                .with_uniform_item_height(gpui_kit::px(44.)),
            network_url_input,
            _network_url_input_subscription: network_url_input_subscription,
            export_in_progress: false,
            export_status: None,
        }
    }
}
