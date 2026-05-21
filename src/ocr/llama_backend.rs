use std::time::Duration;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use image::DynamicImage;
use reqwest::blocking::Client;
use serde_json::{Value, json};

use crate::ocr::{OcrBackend, OcrConfig, OcrPageResult};

const DEFAULT_PROMPT: &str = "Text Recognition:";
const DEFAULT_TIMEOUT_SECS: u64 = 120;
const DEFAULT_MAX_TOKENS: u32 = 4096;

/// OCR backend that delegates inference to a llama-server VLM via the
/// OpenAI-compatible `/v1/chat/completions` endpoint.
///
/// Images are JPEG-compressed with turbojpeg before transmission.
/// The HTTP client is synchronous - callers must invoke `run` from a
/// thread-pool context (rayon worker or `thread::spawn`), never from an
/// async executor thread.
///
/// Construct once and share via `Arc<dyn OcrBackend>`.
///
/// # Example
///
/// ```no_run
/// # use docuparse::ocr::{LlamaServerBackend, LlamaServerConfig};
/// # use std::time::Duration;
/// let backend = LlamaServerBackend::new("http://localhost:8080/v1/chat/completions");
///
/// // or with full config:
/// let backend = LlamaServerBackend::from_config(
///     LlamaServerConfig::builder()
///         .endpoint("http://localhost:8080/v1/chat/completions")
///         .timeout(Duration::from_secs(300))
///         .prompt("Extract all text verbatim:")
///         .build(),
/// );
/// ```
pub struct LlamaServerBackend {
    client: Client,
    endpoint: String,
    prompt: String,
    max_tokens: u32,
}

/// Construction parameters for [`LlamaServerBackend`].
///
/// All fields except `endpoint` have sensible defaults - use
/// [`LlamaServerBackend::new`] when defaults are acceptable.
#[derive(bon::Builder)]
#[builder(on(String, into))]
pub struct LlamaServerConfig {
    /// Full URL of the `/v1/chat/completions` endpoint.
    endpoint: String,

    /// Instruction sent to the VLM alongside the image.
    /// Tune per-model - GLM-OCR, Qwen2.5-VL, and LLaVA respond differently
    /// to the same prompt.
    #[builder(default = DEFAULT_PROMPT.to_string())]
    prompt: String,

    /// Per-request HTTP timeout.
    /// Set to match your worst-case page latency on the target hardware.
    #[builder(default = Duration::from_secs(DEFAULT_TIMEOUT_SECS))]
    timeout: Duration,

    /// Maximum tokens the VLM may generate per page.
    #[builder(default = DEFAULT_MAX_TOKENS)]
    max_tokens: u32,
}

impl LlamaServerBackend {
    /// Construct with default prompt, timeout, and max-token limit.
    ///
    /// `endpoint` is the full chat completions URL,
    /// e.g. `"http://localhost:8080/v1/chat/completions"`.
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self::from_config(LlamaServerConfig::builder().endpoint(endpoint).build())
    }

    /// Construct from explicit config. Use when defaults need overriding.
    pub fn from_config(config: LlamaServerConfig) -> Self {
        let client = Client::builder()
            .timeout(config.timeout)
            .build()
            .expect("reqwest client construction is infallible under normal conditions");

        Self {
            client,
            endpoint: config.endpoint,
            prompt: config.prompt,
            max_tokens: config.max_tokens,
        }
    }

    /// Encode `jpeg_bytes` as a data URI, POST to llama-server, and extract
    /// the generated text from the OpenAI-compatible response.
    fn encode_and_send(
        &self,
        jpeg_bytes: &[u8],
        page_index: u32,
    ) -> Result<OcrPageResult, Box<dyn std::error::Error + Send + Sync>> {
        let prefix = "data:image/jpeg;base64,";
        // base64 expands every 3 input bytes to 4 output chars; +4 absorbs ceiling rounding.
        let mut data_uri = String::with_capacity(prefix.len() + (jpeg_bytes.len() * 4 / 3) + 4);
        data_uri.push_str(prefix);
        STANDARD.encode_string(jpeg_bytes, &mut data_uri);

        let payload = json!({
            "messages": [{
                "role": "user",
                "content": [
                    { "type": "image_url", "image_url": { "url": data_uri } },
                    { "type": "text", "text": self.prompt }
                ]
            }],
            "max_tokens": self.max_tokens,
            "temperature": 0.0  // deterministic decoding - must not be tunable for OCR
        });

        let response = self.client.post(&self.endpoint).json(&payload).send()?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            return Err(format!("llama-server returned {status}: {body}").into());
        }

        let json_resp: Value = response.json()?;
        let text = json_resp["choices"][0]["message"]["content"]
            .as_str()
            .ok_or("missing choices[0].message.content in llama-server response")?
            .to_string();

        Ok(OcrPageResult { page_index, text })
    }
}

impl OcrBackend for LlamaServerBackend {
    fn run(
        &self,
        image: &DynamicImage,
        page_index: u32,
        _config: &OcrConfig,
    ) -> Result<OcrPageResult, Box<dyn std::error::Error + Send + Sync>> {
        // Borrow the existing Rgb8 buffer if available; allocate only if the
        // image is in a different format. turbojpeg takes &RgbImage.
        let rgb8 = image
            .as_rgb8()
            .map(std::borrow::Cow::Borrowed)
            .unwrap_or_else(|| std::borrow::Cow::Owned(image.to_rgb8()));

        // Quality 85 balances VLM input fidelity against HTTP payload size.
        let jpeg = turbojpeg::compress_image(&*rgb8, 85, turbojpeg::Subsamp::Sub2x2)
            .map_err(|e| format!("turbojpeg compression failed: {e}"))?;

        self.encode_and_send(&jpeg, page_index)
    }
}
