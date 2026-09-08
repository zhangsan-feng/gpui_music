use gpui_kit::http_client::Url;
use std::collections::HashMap;
use std::fmt::Debug;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone)]
pub struct NetworkStatic {
    pub id: String,
    pub name: String,
    pub img: String,
    pub author: String,
    pub category: String,
    pub source: String,
    pub headers: reqwest::header::HeaderMap,
    pub extra: HashMap<String, serde_json::Value>,
    pub func: Arc<dyn NetworkStaticInterface + Send + Sync>,
}

impl Default for NetworkStatic {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            img: String::new(),
            author: String::new(),
            category: String::new(),
            source: String::new(),
            headers: reqwest::header::HeaderMap::new(),
            extra: HashMap::new(),
            func: Arc::new(LocalStatic),
        }
    }
}

impl Debug for NetworkStatic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetworkStatic")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("img", &self.img)
            .field("author", &self.author)
            .field("category", &self.category)
            .field("source", &self.source)
            .finish()
    }
}

pub trait NetworkStaticInterface {
    fn download(&self, params: &NetworkStatic);
    fn play(&self, params: &NetworkStatic) -> String;
    fn detail(&self, params: &NetworkStatic) -> Vec<NetworkStatic>;
}

pub struct LocalStatic;

impl NetworkStaticInterface for LocalStatic {
    fn download(&self, _params: &NetworkStatic) {}

    fn play(&self, params: &NetworkStatic) -> String {
        let source = params.source.trim();
        if source.contains("://") {
            return source.to_string();
        }

        Url::from_file_path(Path::new(source))
            .map(|uri| uri.to_string())
            .unwrap_or_else(|_| source.to_string())
    }

    fn detail(&self, _params: &NetworkStatic) -> Vec<NetworkStatic> {
        Vec::new()
    }
}
