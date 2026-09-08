use crate::PlayStatic;
use gpui_kit::{
    AppContext, Context, Entity, EntityId, EventEmitter, Global, Subscription, WindowId,
};

#[derive(Clone)]
pub struct PlayCoreState {}
impl PlayCoreState {
    pub fn new(_: &mut Context<Self>) -> Self {
        PlayCoreState {}
    }
}

pub enum PlayCoreStateEvent {
    TogglePlay(WindowId, EntityId, PlayStatic),
    PlayBackFished(WindowId, EntityId, PlayStatic),
}
impl EventEmitter<PlayCoreStateEvent> for PlayCoreState {}

pub struct PlayCoreGlobalState(pub(crate) Entity<PlayCoreState>);

impl PlayCoreGlobalState {
    pub fn new(state: Entity<PlayCoreState>) -> Self {
        Self(state)
    }

    pub fn publish<C: AppContext>(cx: &mut C, event: PlayCoreStateEvent) {
        let state = cx.read_global::<Self, _>(|state, _| state.0.clone());
        state.update(cx, |_, cx| cx.emit(event));
    }

    pub fn subscribe<T: 'static>(
        cx: &mut Context<T>,
        mut on_event: impl FnMut(&mut T, &PlayCoreStateEvent, &mut Context<T>) + 'static,
    ) -> Subscription {
        let state = cx.global::<Self>().0.clone();
        cx.subscribe(&state, move |this, _, event, cx| {
            on_event(this, event, cx);
        })
    }
}

impl Global for PlayCoreGlobalState {}
