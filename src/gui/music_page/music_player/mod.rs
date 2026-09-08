mod core;
mod ui;

use crate::drive::NetworkStatic;
use gpui_kit::{AppContext, Context, Entity, Window};
use player_core::PlayCore;

pub struct MusicPlayer {
    play_core: Entity<PlayCore>,
    play_list: Vec<NetworkStatic>,
    current_index: Option<usize>,
}

impl MusicPlayer {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let play_core = cx.new(|cx| PlayCore::new(window, cx));
        cx.observe(&play_core, |_, _, cx| cx.notify()).detach();

        Self {
            play_core,
            play_list: Vec::new(),
            current_index: None,
        }
    }
}
