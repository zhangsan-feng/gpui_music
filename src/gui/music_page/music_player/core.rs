use super::MusicPlayer;
use crate::drive::NetworkStatic;
use gpui_kit::{AsyncApp, Context, EntityId, WindowId};
use player_core::{PlayCoreGlobalState, PlayCoreStateEvent, PlayStatic};

impl MusicPlayer {
    pub fn _play_core_id(&self) -> EntityId {
        self.play_core.entity_id()
    }

    pub fn _set_play_list(&mut self, play_list: Vec<NetworkStatic>, cx: &mut Context<Self>) {
        let current_id = self
            .current_index
            .and_then(|index| self.play_list.get(index))
            .map(|item| item.id.clone());
        self.play_list = play_list;
        self.current_index =
            current_id.and_then(|id| self.play_list.iter().position(|item| item.id == id));
        cx.notify();
    }

    pub fn _is_current_item(&self, id: &str, cx: &gpui_kit::App) -> bool {
        self.play_core.read(cx)._view_state().player.id == id
    }

    pub fn _toggle_play(&mut self, window_id: WindowId, cx: &mut Context<Self>) {
        let Some(index) = self
            .current_index
            .or_else(|| (!self.play_list.is_empty()).then_some(0))
        else {
            return;
        };

        if self.current_index.is_none() {
            self._play_item(index, window_id, cx);
            return;
        }

        self.play_core
            .update(cx, |player, cx| player._toggle_play(cx));
    }

    pub fn _play_item(&mut self, index: usize, window_id: WindowId, cx: &mut Context<Self>) {
        let Some(data) = self.play_list.get(index).cloned() else {
            return;
        };

        self.current_index = Some(index);
        let play_core_id = self.play_core.entity_id();
        let mut cx_async = cx.to_async().clone();
        let source_data = data.clone();
        cx.spawn(move |_, _: &mut AsyncApp| async move {
            let source = tokio::spawn(async move { source_data.func.play(&source_data) })
                .await
                .unwrap_or_default();
            if source.trim().is_empty() {
                return;
            }

            PlayCoreGlobalState::publish(
                &mut cx_async,
                PlayCoreStateEvent::TogglePlay(
                    window_id,
                    play_core_id,
                    PlayStatic {
                        id: data.id,
                        title: data.name,
                        url: source,
                        headers: data.headers,
                    },
                ),
            );
        })
        .detach();
        cx.notify();
    }

    pub fn _play_previous(&mut self, window_id: WindowId, cx: &mut Context<Self>) {
        let Some(index) = self.current_index else {
            return;
        };
        if let Some(previous) = index.checked_sub(1) {
            self._play_item(previous, window_id, cx);
        }
    }

    pub fn _play_next(&mut self, window_id: WindowId, cx: &mut Context<Self>) {
        let Some(index) = self.current_index else {
            return;
        };
        if index + 1 < self.play_list.len() {
            self._play_item(index + 1, window_id, cx);
        }
    }
}
