use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;

use super::{Tool, ToolContext};

#[derive(Debug, Deserialize)]
struct WebFetchParams {
    /// The URL to fetch content from
    url: String,
    /// Maximum content length to return (in characters). Defaults to 50000.
    #[serde(default = "default_max_length")]
    max_length: usize,
    /// Whether to extract just the text content (strip HTML). Defaults to true.
    #[serde(default = "default_extract_text")]
    extract_text: bool,
}

fn default_max_length() -> usize {
    50000
}

fn default_extract_text() -> bool {
    true
}

pub struct WebFetchTool;

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "webfetch"
    }

    fn description(&self) -> &str {
        "Fetches content from a URL and returns it as text. \
         Can extract text from HTML pages or return raw content. \
         Useful for reading documentation, API responses, or web pages."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to fetch content from"
                },
                "max_length": {
                    "type": "integer",
                    "description": "Maximum content length to return in characters. Defaults to 50000."
                },
                "extract_text": {
                    "type": "boolean",
                    "description": "Whether to extract just the text content (strip HTML). Defaults to true."
                }
            },
            "required": ["url"]
        })
    }

    async fn execute(&self, params: serde_json::Value, _ctx: &ToolContext<'_>) -> Result<String> {
        let params: WebFetchParams = serde_json::from_value(params)?;

        // Validate URL
        let url = match url::Url::parse(&params.url) {
            Ok(u) => u,
            Err(e) => return Ok(format!("Invalid URL: {}", e)),
        };

        // Only allow http and https
        if url.scheme() != "http" && url.scheme() != "https" {
            return Ok(format!("Only HTTP and HTTPS URLs are supported"));
        }

        // Fetch the content
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("SafeCoder/1.0")
            .build()?;

        let response = match client.get(url.as_str()).send().await {
            Ok(r) => r,
            Err(e) => return Ok(format!("Failed to fetch URL: {}", e)),
        };

        let status = response.status();
        if !status.is_success() {
            return Ok(format!(
                "HTTP error: {} {}",
                status.as_u16(),
                status.canonical_reason().unwrap_or("Unknown")
            ));
        }

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        let body = match response.text().await {
            Ok(t) => t,
            Err(e) => return Ok(format!("Failed to read response body: {}", e)),
        };

        // Extract text if requested and content is HTML
        let content = if params.extract_text && content_type.contains("text/html") {
            extract_text_from_html(&body)
        } else {
            body
        };

        // Truncate if needed
        let truncated = if content.len() > params.max_length {
            format!(
                "{}\n\n... [Content truncated at {} characters]",
                &content[..params.max_length],
                params.max_length
            )
        } else {
            content
        };

        Ok(format!(
            "Fetched {} ({} characters):\n\n{}",
            params.url,
            truncated.len(),
            truncated
        ))
    }
}

/// Simple HTML to text extraction
fn extract_text_from_html(html: &str) -> String {
    // Remove script and style tags with their content
    let re_script = regex::Regex::new(r"(?is)<script[^>]*>.*?</script>").unwrap();
    let re_style = regex::Regex::new(r"(?is)<style[^>]*>.*?</style>").unwrap();
    let re_comments = regex::Regex::new(r"(?s)<!--.*?-->").unwrap();

    let text = re_script.replace_all(html, "");
    let text = re_style.replace_all(&text, "");
    let text = re_comments.replace_all(&text, "");

    // Replace common block elements with newlines
    let re_blocks = regex::Regex::new(r"(?i)</?(p|div|br|h[1-6]|li|tr)[^>]*>").unwrap();
    let text = re_blocks.replace_all(&text, "\n");

    // Remove all remaining HTML tags
    let re_tags = regex::Regex::new(r"<[^>]+>").unwrap();
    let text = re_tags.replace_all(&text, "");

    // Decode common HTML entities
    let text = text
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'");

    // Clean up whitespace
    let re_whitespace = regex::Regex::new(r"\n\s*\n+").unwrap();
    let text = re_whitespace.replace_all(&text, "\n\n");

    text.trim().to_string()
}
