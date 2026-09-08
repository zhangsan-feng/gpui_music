use super::VideoPage;
use gpui_kit::{AnyElement, Context, Window};

impl VideoPage {
    pub(super) fn render_recommend_page(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let empty_text = if self.is_loading {
            "加载中"
        } else {
            "暂无推荐内容"
        };
        self.render_video_list(
            self.recommend_result.clone(),
            "video-recommend-list",
            empty_text,
            window,
            cx,
        )
    }
}
