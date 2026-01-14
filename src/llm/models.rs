// Central registry of supported LLM model names for use in selection UIs, modals, etc.
// Extend as needed when adding/removing providers
// Used for modal model picker in CLI/TUI

pub fn available_models() -> &'static [&'static str] {
    // Add/remove models here as supported
    &[
        // Anthropic Claude
        "claude-3-opus-20240229",
        "claude-3-sonnet-20240229",
        "claude-3-haiku-20240307",
        // OpenAI
        "gpt-4o",
        "gpt-4",
        "gpt-4-32k",
        "gpt-3.5-turbo",
        // OpenRouter top models (sample)
        "openrouter/anthropic/claude-3-opus",
        "openrouter/openai/gpt-4o",
        "openrouter/google/gemini-pro",
        // Ollama - add any local models you support
        "llama3",
        "mistral",
        "phi3",
        
        // Add more as desired...
    ]
}
