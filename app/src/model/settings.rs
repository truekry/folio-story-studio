//! Endpoint, model, context window and the prompts the author can edit.

use super::*;

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct AiSettings {
    #[serde(default = "default_base")]
    pub base_url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default = "default_summary_window")]
    pub summary_window: usize,
    /// The context window the endpoint reported, in tokens. Zero means it did
    /// not say, and Folio sends its usual reply length.
    #[serde(default)]
    pub context_limit: usize,
    /// What each AI function is asked to do, and how long its answer should be.
    /// Older libraries have no such block and get the built-in prompts.
    #[serde(default)]
    pub prompts: PromptSettings,
}

/// The instruction for one AI function, as the author left it. An empty text
/// means "use the built-in default", so a later version can still improve its
/// own wording. A missing word target means the same; a target of zero turns
/// the length request off for this function.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct PromptSetting {
    #[serde(default)]
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub words: Option<u32>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct PromptSettings {
    #[serde(default)]
    pub summary: PromptSetting,
    #[serde(default, rename = "continue")]
    pub continuing: PromptSetting,
    #[serde(default)]
    pub rewrite: PromptSetting,
    #[serde(default)]
    pub elaborate: PromptSetting,
    #[serde(default)]
    pub brainstorm: PromptSetting,
}

pub(crate) fn default_base() -> String {
    "https://api.openai.com/v1".to_string()
}

pub(crate) fn default_model() -> String {
    "gpt-4o-mini".to_string()
}

pub(crate) fn default_temperature() -> f32 {
    0.8
}

pub(crate) fn default_max_tokens() -> u32 {
    700
}

/// How many earlier chapter summaries travel as themselves. Everything older is
/// condensed into one story-so-far block.
pub const DEFAULT_SUMMARY_WINDOW: usize = 3;

pub const MAX_SUMMARY_WINDOW: usize = 10;

pub(crate) fn default_summary_window() -> usize {
    DEFAULT_SUMMARY_WINDOW
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            base_url: default_base(),
            api_key: String::new(),
            model: default_model(),
            temperature: default_temperature(),
            max_tokens: default_max_tokens(),
            summary_window: default_summary_window(),
            context_limit: 0,
            prompts: PromptSettings::default(),
        }
    }
}

/// True for addresses on the author's own machine or network, which usually run
/// an OpenAI-compatible server without any key.
pub fn is_private_address(url: &str) -> bool {
    let host = host_of(url);
    if host.is_empty() {
        return false;
    }
    if host == "localhost"
        || host == "host.docker.internal"
        || host == "::1"
        || host == "[::1]"
        || host.ends_with(".local")
    {
        return true;
    }
    // A bare machine name such as "workstation:5001" is on the network too.
    if !host.contains('.') {
        return true;
    }
    match host.parse::<std::net::Ipv4Addr>() {
        Ok(ip) => ip.is_loopback() || ip.is_private() || ip.is_link_local() || ip.is_unspecified(),
        Err(_) => false,
    }
}

/// The host part of an address, with any scheme, user and port removed.
pub(crate) fn host_of(url: &str) -> String {
    let rest = match url.find("://") {
        Some(at) => &url[at + 3..],
        None => url,
    };
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..end];
    let authority = match authority.rfind('@') {
        Some(at) => &authority[at + 1..],
        None => authority,
    };
    if let Some(close) = authority.find(']') {
        return authority[..=close].to_ascii_lowercase();
    }
    authority
        .split(':')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
}

impl AiSettings {
    /// The demo writer stands in when no address is given at all, and when a
    /// cloud service is addressed without a key. A machine on the author's own
    /// network may well run without one.
    pub fn is_demo(&self) -> bool {
        let base = self.base_url.trim();
        if base.is_empty() {
            return true;
        }
        if !self.api_key.trim().is_empty() {
            return false;
        }
        !is_private_address(base)
    }

    pub fn label(&self) -> String {
        if self.is_demo() {
            "Demo writer".to_string()
        } else if self.model.trim().is_empty() {
            "Connected".to_string()
        } else {
            self.model.clone()
        }
    }

    /// The window actually used: a hand-edited library file cannot ask for more
    /// summaries than the settings window offers.
    pub fn recent_window(&self) -> usize {
        self.summary_window.min(MAX_SUMMARY_WINDOW)
    }
}
