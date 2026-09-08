use super::VideoPage;
use gpui_kit::{AnyElement, Context, Window};

impl VideoPage {
    pub(super) fn render_search_page(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let items = self
            .search_result
            .values()
            .flat_map(|videos| videos.iter().cloned())
            .collect();
        let empty_text = if self.is_searching {
            "搜索中"
        } else if self.is_loading {
            "加载中"
        } else {
            "暂无搜索结果"
        };

        self.render_video_list(items, "video-search-list", empty_text, window, cx)
    }
}
