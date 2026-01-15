//! Model picker modal component for /model command
//!
//! Provides a visual model selector that appears when running /model without arguments.

use crate::config::LlmProvider;

/// A model entry in the picker
#[derive(Debug, Clone)]
pub struct ModelEntry {
    /// Display name
    pub name: String,
    /// Model ID (what gets set in config)
    pub id: String,
    /// Optional description
    pub description: Option<String>,
    /// Whether this is the currently active model
    pub is_active: bool,
}

/// Model picker state
#[derive(Debug, Clone)]
pub struct ModelPicker {
    /// Whether the picker is visible
    pub visible: bool,
    /// Current provider
    pub provider: LlmProvider,
    /// List of available models
    pub models: Vec<ModelEntry>,
    /// Currently selected index
    pub selected: usize,
    /// Filter/search text
    pub filter: String,
    /// Currently active model name
    pub active_model: String,
}

impl Default for ModelPicker {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelPicker {
    pub fn new() -> Self {
        Self {
            visible: false,
            provider: LlmProvider::GitHubCopilot,
            models: Vec::new(),
            selected: 0,
            filter: String::new(),
            active_model: String::new(),
        }
    }

    /// Open the model picker with models for the given provider
    pub fn open(&mut self, provider: LlmProvider, active_model: &str) {
        self.visible = true;
        self.provider = provider.clone();
        self.active_model = active_model.to_string();
        self.filter.clear();
        self.selected = 0;
        self.load_models(provider, active_model);
    }

    /// Close the model picker
    pub fn close(&mut self) {
        self.visible = false;
        self.models.clear();
        self.filter.clear();
    }

    /// Load models for the given provider
    pub fn load_models(&mut self, provider: LlmProvider, active_model: &str) {
        self.models.clear();

        let models = get_models_for_provider(&provider);

        for (name, id, desc) in models {
            self.models.push(ModelEntry {
                name: name.to_string(),
                id: id.to_string(),
                description: desc.map(|s| s.to_string()),
                is_active: id == active_model,
            });
        }

        // Select the active model by default
        for (i, model) in self.models.iter().enumerate() {
            if model.is_active {
                self.selected = i;
                break;
            }
        }
    }

    /// Get filtered models based on current filter
    pub fn filtered_models(&self) -> Vec<&ModelEntry> {
        if self.filter.is_empty() {
            self.models.iter().collect()
        } else {
            let filter_lower = self.filter.to_lowercase();
            self.models
                .iter()
                .filter(|m| {
                    m.name.to_lowercase().contains(&filter_lower)
                        || m.id.to_lowercase().contains(&filter_lower)
                })
                .collect()
        }
    }

    /// Update filter text
    pub fn set_filter(&mut self, filter: String) {
        self.filter = filter;
        self.selected = 0;
    }

    /// Add character to filter
    pub fn add_filter_char(&mut self, c: char) {
        self.filter.push(c);
        self.selected = 0;
    }

    /// Remove last character from filter
    pub fn backspace_filter(&mut self) {
        self.filter.pop();
    }

    /// Move selection up
    pub fn move_up(&mut self) {
        let filtered = self.filtered_models();
        if !filtered.is_empty() && self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Move selection down
    pub fn move_down(&mut self) {
        let filtered = self.filtered_models();
        if !filtered.is_empty() && self.selected < filtered.len() - 1 {
            self.selected += 1;
        }
    }

    /// Get the currently selected model
    pub fn get_selected(&self) -> Option<&ModelEntry> {
        let filtered = self.filtered_models();
        filtered.get(self.selected).copied()
    }

    /// Get the selected model ID
    pub fn get_selected_id(&self) -> Option<String> {
        self.get_selected().map(|m| m.id.clone())
    }
}

/// Get available models for a provider
fn get_models_for_provider(provider: &LlmProvider) -> Vec<(&'static str, &'static str, Option<&'static str>)> {
    match provider {
        LlmProvider::GitHubCopilot => vec![
            ("GPT-5 Mini", "gpt-5-mini", Some("Fast GPT-5 model")),
            ("GPT-5", "gpt-5", Some("OpenAI GPT-5")),
            ("GPT-5.1", "gpt-5.1", Some("Enhanced GPT-5")),
            ("GPT-5.1-Codex", "gpt-5.1-codex", Some("Code-optimized GPT-5.1")),
            ("GPT-5.1-Codex-Mini", "gpt-5.1-codex-mini", Some("Fast code model")),
            ("GPT-5.1-Codex-Max", "gpt-5.1-codex-max", Some("Most capable code model")),
            ("GPT-5-Codex (Preview)", "gpt-5-codex", Some("GPT-5 Codex preview")),
            ("Grok Code Fast 1", "grok-code-fast-1", Some("xAI Grok for code")),
            ("Claude Sonnet 4", "claude-sonnet-4", Some("Anthropic Claude Sonnet 4")),
            ("Claude Sonnet 4.5", "claude-sonnet-4.5", Some("Latest Claude Sonnet")),
            ("GPT-4o", "gpt-4o", Some("GPT-4o model")),
            ("o1", "o1", Some("OpenAI reasoning model")),
            ("o3-mini", "o3-mini", Some("Mini reasoning model")),
        ],
        LlmProvider::Anthropic => vec![
            ("Claude Sonnet 4", "claude-sonnet-4-20250514", Some("Latest Claude Sonnet")),
            ("Claude 3.5 Sonnet", "claude-3-5-sonnet-20241022", Some("Previous generation Sonnet")),
            ("Claude 3.5 Haiku", "claude-3-5-haiku-20241022", Some("Fast and efficient")),
            ("Claude Opus 4", "claude-opus-4-20250514", Some("Most capable Claude")),
        ],
        LlmProvider::OpenAI => vec![
            ("GPT-4o", "gpt-4o", Some("Latest GPT-4o")),
            ("GPT-4o Mini", "gpt-4o-mini", Some("Faster, cheaper")),
            ("GPT-4 Turbo", "gpt-4-turbo", Some("High performance")),
            ("o1", "o1", Some("Reasoning model")),
            ("o1-mini", "o1-mini", Some("Smaller reasoning")),
        ],
        LlmProvider::OpenRouter => vec![
            ("Claude Sonnet 4", "anthropic/claude-sonnet-4", Some("Via OpenRouter")),
            ("GPT-4o", "openai/gpt-4o", Some("Via OpenRouter")),
            ("Llama 3.1 70B", "meta-llama/llama-3.1-70b-instruct", Some("Open source")),
            ("Mistral Large", "mistralai/mistral-large-latest", Some("Mistral's flagship")),
            ("DeepSeek V3", "deepseek/deepseek-chat", Some("DeepSeek's latest")),
        ],
        LlmProvider::Ollama => vec![
            ("Llama 3.2", "llama3.2", Some("Latest Llama")),
            ("CodeLlama", "codellama", Some("Code-focused")),
            ("Mistral", "mistral", Some("Mistral 7B")),
            ("DeepSeek Coder", "deepseek-coder", Some("Code specialist")),
            ("Qwen 2.5 Coder", "qwen2.5-coder", Some("Alibaba's coder")),
        ],
    }
}
