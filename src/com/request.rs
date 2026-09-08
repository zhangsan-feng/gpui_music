use futures_util::StreamExt;
use gpui_kit::http_client::http::HeaderMap;
use log::error;
use reqwest::{ClientBuilder, Response, multipart};
use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::io::AsyncWriteExt;

const MAX_REQUEST_ATTEMPTS: usize = 3;
const RETRY_DELAYS: [Duration; MAX_REQUEST_ATTEMPTS - 1] =
    [Duration::from_millis(500), Duration::from_secs(1)];

trait ResponseHandler {
    async fn handle(self) -> anyhow::Result<serde_json::Value, anyhow::Error>;
}

impl ResponseHandler for reqwest::Response {
    async fn handle(self) -> anyhow::Result<serde_json::Value, anyhow::Error> {
        let status = self.status();
        let bytes = match self.bytes().await {
            Ok(bytes) => bytes,
            Err(err) => {
                error!("读取响应失败: {}{}", err, timeout_suffix(&err));
                return Err(anyhow::anyhow!("读取响应失败: {}", err));
            }
        };
        // let body_str = String::from_utf8_lossy(&bytes);

        if status.is_success() {
            match serde_json::from_slice(&bytes) {
                Ok(data) => Ok(data),
                Err(err) => {
                    error!("响应序列化失败: {}", err);
                    // Err(anyhow::anyhow!("序列化失败: {}, 响应内容: {}", err, body_str))
                    Err(anyhow::anyhow!("序列化失败: {}", err))
                }
            }
        } else {
            error!("请求失败, 状态码: {}", status);
            // Err(anyhow::anyhow!("请求失败, 状态码: {}, 响应: {}", status, body_str))
            Err(anyhow::anyhow!("序列化失败: {}", status))
        }
    }
}

pub struct HttpClient {
    client: Arc<reqwest::Client>,
}

fn timeout_suffix(error: &reqwest::Error) -> &'static str {
    if error.is_timeout() {
        " (请求超时)"
    } else {
        ""
    }
}

fn should_retry_status(status: reqwest::StatusCode) -> bool {
    matches!(status.as_u16(), 408 | 429 | 500..=599)
}

// static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(reqwest::Client::new);

impl HttpClient {
    pub fn new() -> Self {
        static CLIENT: OnceLock<Arc<reqwest::Client>> = OnceLock::new();

        Self {
            client: CLIENT
                .get_or_init(|| {
                    Arc::new(
                        ClientBuilder::new()
                            .timeout(Duration::from_secs(15))
                            .connect_timeout(Duration::from_secs(8))
                            .build()
                            .unwrap(),
                    )
                })
                .clone(),
        }
    }

    async fn send_with_retry<F>(
        &self,
        method: &str,
        url: &str,
        mut build_request: F,
    ) -> anyhow::Result<Response, anyhow::Error>
    where
        F: FnMut() -> reqwest::RequestBuilder,
    {
        for attempt in 0..MAX_REQUEST_ATTEMPTS {
            let attempt_number = attempt + 1;
            match build_request().send().await {
                Ok(response) if should_retry_status(response.status()) => {
                    if attempt_number == MAX_REQUEST_ATTEMPTS {
                        return Ok(response);
                    }

                    log::warn!(
                        "{}请求返回可重试状态 [{}], 第 {}/{} 次尝试",
                        method,
                        response.status(),
                        attempt_number,
                        MAX_REQUEST_ATTEMPTS
                    );
                }
                Ok(response) => return Ok(response),
                Err(error) => {
                    if attempt_number == MAX_REQUEST_ATTEMPTS {
                        return Err(error.into());
                    }

                    log::warn!(
                        "{}请求失败 [{}]: {}{}, 第 {}/{} 次重试",
                        method,
                        url,
                        error,
                        timeout_suffix(&error),
                        attempt_number,
                        MAX_REQUEST_ATTEMPTS
                    );
                }
            }

            tokio::time::sleep(RETRY_DELAYS[attempt]).await;
        }

        unreachable!("request attempts must be greater than zero");
    }

    pub async fn download_file(
        &self,
        file_name: String,
        url: String,
        header: HeaderMap,
    ) -> anyhow::Result<()> {
        if Path::new(&file_name).exists() {
            return Ok(());
        }

        println!("当前下载: {}", file_name);
        let response = match self
            .send_with_retry("下载", &url, || {
                self.client.get(&url).headers(header.clone())
            })
            .await
        {
            Ok(response) => response,
            Err(err) => {
                error!("下载请求失败 [{}]: {}", url, err);
                return Err(err);
            }
        };

        if !response.status().is_success() {
            error!("下载请求失败 [{}], 状态码: {}", url, response.status());
            return Err(anyhow::anyhow!("response not 200 "));
        }

        let mut file = tokio::fs::File::create(&file_name).await?;
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = match chunk {
                Ok(chunk) => chunk,
                Err(err) => {
                    error!(
                        "下载数据读取失败 [{}]: {}{}",
                        url,
                        err,
                        timeout_suffix(&err)
                    );
                    return Err(err.into());
                }
            };
            file.write_all(&chunk).await?;
        }

        file.flush().await?;
        println!("下载完成: {}", file_name);
        Ok(())
    }

    pub async fn get_for_html(
        &self,
        url: &str,
        header: HeaderMap,
    ) -> anyhow::Result<Response, anyhow::Error> {
        let response = match self
            .send_with_retry("GET", url, || self.client.get(url).headers(header.clone()))
            .await
        {
            Ok(r) => r,
            Err(e) => {
                error!("GET请求失败 [{}]: {}", url, e);
                return Err(anyhow::anyhow!("GET请求失败: {}", e));
            }
        };

        if !response.status().is_success() {
            error!("GET请求失败 [{}], 状态码: {}", url, response.status());
        }

        Ok(response)
    }

    pub async fn get(
        &self,
        url: &str,
        header: HeaderMap,
    ) -> anyhow::Result<serde_json::Value, anyhow::Error> {
        let response = match self
            .send_with_retry("GET", url, || self.client.get(url).headers(header.clone()))
            .await
        {
            Ok(r) => r,
            Err(e) => {
                error!("GET请求失败 [{}]: {}", url, e);
                return Err(anyhow::anyhow!("GET请求失败: {}", e));
            }
        };

        response.handle().await
    }

    pub async fn post(
        &self,
        url: &str,
        header: HeaderMap,
        body: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value, anyhow::Error> {
        let response = match self
            .send_with_retry("POST", url, || {
                self.client.post(url).headers(header.clone()).json(&body)
            })
            .await
        {
            Ok(r) => r,
            Err(e) => {
                error!("POST请求失败 [{}]: {}", url, e);
                return Err(anyhow::anyhow!("POST请求失败: {}", e));
            }
        };

        response.handle().await
    }

    pub async fn post_form(
        &self,
        url: String,
        form: multipart::Form,
    ) -> anyhow::Result<serde_json::Value, anyhow::Error> {
        let response = match self.client.post(&url).multipart(form).send().await {
            Ok(r) => r,
            Err(e) => {
                error!("POST表单请求失败 [{}]: {}{}", url, e, timeout_suffix(&e));
                return Err(anyhow::anyhow!("POST表单请求失败: {}", e));
            }
        };
        response.handle().await
    }
}
