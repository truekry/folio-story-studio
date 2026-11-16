//! Finding and talking to the endpoint: addresses, model lists and context windows.

use super::*;

/// The address as typed, with a scheme if the author left it out. Local servers
/// are usually noted down as "192.168.10.20:5001/v1", which no HTTP client
/// understands on its own.
pub(crate) fn with_scheme(base: &str) -> String {
    let base = base.trim();
    if base.is_empty() {
        return String::new();
    }
    let rest = match base.find("://") {
        Some(at) => &base[at + 3..],
        None => base,
    };
    if crate::model::is_private_address(base) {
        // The machines that run Ollama, LM Studio or KoboldCpp speak plain
        // HTTP. A leftover https:// (the OpenAI preset is a common source)
        // meets them with a TLS handshake that fails as "corrupt message".
        format!("http://{rest}")
    } else if base.contains("://") {
        base.to_string()
    } else {
        format!("https://{base}")
    }
}

/// The server root, without the API path: "http://host:5001" for a base of
/// "host:5001/v1". Local servers keep their context size next to the API.
pub(crate) fn origin(base: &str) -> String {
    let full = with_scheme(base);
    let full = full.trim_end_matches('/');
    match full.find("://") {
        Some(at) => match full[at + 3..].find('/') {
            Some(slash) => full[..at + 3 + slash].to_string(),
            None => full.to_string(),
        },
        None => full.to_string(),
    }
}

pub(crate) fn endpoint(base: &str) -> String {
    let base = with_scheme(base);
    let base = base.trim_end_matches('/');
    if base.contains("/chat/completions") {
        base.to_string()
    } else {
        format!("{base}/chat/completions")
    }
}

pub(crate) fn models_endpoint(base: &str) -> String {
    let base = with_scheme(base);
    let base = base.trim_end_matches('/');
    // A pasted chat route is a valid address for `endpoint`, so it must also
    // lead to the catalogue: /v1/chat/completions -> /v1/models, not
    // /v1/chat/completions/models.
    let base = base.strip_suffix("/chat/completions").unwrap_or(base);
    if base.ends_with("/models") {
        base.to_string()
    } else {
        format!("{base}/models")
    }
}

/// Read the model names out of a `/models` answer. The OpenAI shape is
/// `{"data": [{"id": …}]}`; Ollama's own endpoint answers `{"models": [{"name": …}]}`.
pub(crate) fn parse_models(text: &str) -> Result<Vec<String>, String> {
    let value: Value = serde_json::from_str(text).map_err(|_| {
        format!(
            "The endpoint sent something that is not JSON: {}",
            trim_chars(text, 200)
        )
    })?;
    let mut names: Vec<String> = value["data"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|entry| entry["id"].as_str())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    if names.is_empty() {
        names = value["models"]
            .as_array()
            .map(|list| {
                list.iter()
                    .filter_map(|entry| entry["name"].as_str())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
    }
    names.retain(|name| !name.trim().is_empty());
    if names.is_empty() {
        return Err("The endpoint answered but listed no models.".to_string());
    }
    names.sort_by_key(|name| name.to_ascii_lowercase());
    names.dedup();
    Ok(names)
}

/// Names a server may use for its context window. OpenAI-compatible catalogues
/// (OpenRouter, vLLM) put one on the model entry; llama.cpp and KoboldCpp
/// answer with one of the others.
pub(crate) const CONTEXT_KEYS: [&str; 7] = [
    "max_context_length",
    "context_length",
    "context_window",
    "max_model_len",
    "n_ctx",
    "true_max_context_length",
    "n_ctx_train",
];

/// Find a context size anywhere in a JSON answer, preferring the known keys.
pub(crate) fn context_in(value: &Value) -> Option<usize> {
    match value {
        Value::Object(map) => {
            for key in CONTEXT_KEYS {
                if let Some(size) = map.get(key).and_then(Value::as_u64) {
                    if size > 0 {
                        return Some(size as usize);
                    }
                }
            }
            map.values().find_map(context_in)
        }
        Value::Array(list) => list.iter().find_map(context_in),
        _ => None,
    }
}

/// The plain number KoboldCpp sends, or a JSON body with a context field.
pub(crate) fn context_from_text(text: &str) -> Option<usize> {
    if let Ok(size) = text.trim().parse::<u64>() {
        return (size > 0).then_some(size as usize);
    }
    let value: Value = serde_json::from_str(text).ok()?;
    context_in(&value).or_else(|| match &value {
        // A tiny wrapper such as {"value": 8192} still says what it means.
        Value::Object(map) if map.len() == 1 => map
            .values()
            .find_map(Value::as_u64)
            .filter(|size| *size > 0)
            .map(|size| size as usize),
        _ => None,
    })
}

/// The context window of the selected model, when the catalogue carries one.
pub(crate) fn context_for_model(value: &Value, model: &str) -> Option<usize> {
    let entries = value["data"]
        .as_array()
        .or_else(|| value["models"].as_array())?;
    let wanted = model.trim();
    let pick = entries
        .iter()
        .find(|entry| {
            entry["id"].as_str() == Some(wanted) || entry["name"].as_str() == Some(wanted)
        })
        .or_else(|| entries.first())?;
    context_in(pick)
}

pub(crate) fn get_body(settings: &AiSettings, url: &str) -> Option<String> {
    let mut request = ureq::get(url).timeout(Duration::from_secs(8));
    if !settings.api_key.trim().is_empty() {
        request = request.set(
            "Authorization",
            &format!("Bearer {}", settings.api_key.trim()),
        );
    }
    request
        .call()
        .ok()
        .map(|response| response.into_string().unwrap_or_default())
}

pub(crate) fn post_body(settings: &AiSettings, url: &str) -> Option<String> {
    let mut request = ureq::post(url)
        .set("Content-Type", "application/json")
        .timeout(Duration::from_secs(8));
    if !settings.api_key.trim().is_empty() {
        request = request.set(
            "Authorization",
            &format!("Bearer {}", settings.api_key.trim()),
        );
    }
    request
        .send_string("{}")
        .ok()
        .map(|response| response.into_string().unwrap_or_default())
}

/// Ask a local server how much it can hold. llama.cpp answers `/props`,
/// LM Studio `/api/v0/models`, KoboldCpp `/api/extra/true_max_context_length`
/// (bare number, GET or POST).
pub(crate) fn context_from_server(settings: &AiSettings) -> Option<usize> {
    let base = settings.base_url.trim();
    if base.is_empty() {
        return None;
    }
    let root = origin(base);
    for url in [
        format!("{root}/props"),
        format!("{root}/api/v0/models"),
        format!("{root}/api/extra/true_max_context_length"),
    ] {
        let url_is_catalogue = url.ends_with("/models");
        if let Some(text) = get_body(settings, &url) {
            if url_is_catalogue {
                let found = serde_json::from_str::<Value>(&text)
                    .ok()
                    .and_then(|value| context_for_model(&value, &settings.model));
                if let Some(size) = found {
                    return Some(size);
                }
            } else if let Some(size) = context_from_text(&text) {
                return Some(size);
            }
        }
        if url.ends_with("true_max_context_length") {
            if let Some(size) = post_body(settings, &url).and_then(|text| context_from_text(&text))
            {
                return Some(size);
            }
        }
    }
    None
}

/// What `/models` told us: the names, and the context size if the server
/// mentions one.
pub struct EndpointModels {
    pub names: Vec<String>,
    pub context_limit: Option<usize>,
}

/// Ask the endpoint which models it offers, so the author can pick one instead
/// of guessing its name.
pub fn list_models(settings: &AiSettings) -> Result<EndpointModels, String> {
    let base = settings.base_url.trim();
    if base.is_empty() {
        return Err("Add an endpoint address first.".to_string());
    }
    let url = models_endpoint(base);
    let mut request = ureq::get(&url).timeout(Duration::from_secs(20));
    if !settings.api_key.trim().is_empty() {
        request = request.set(
            "Authorization",
            &format!("Bearer {}", settings.api_key.trim()),
        );
    }
    match request.call() {
        Ok(response) => {
            let text = response.into_string().unwrap_or_default();
            let names = parse_models(&text)?;
            let context_limit = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|value| context_for_model(&value, &settings.model))
                .or_else(|| context_from_server(settings));
            Ok(EndpointModels {
                names,
                context_limit,
            })
        }
        Err(ureq::Error::Status(404, _)) | Err(ureq::Error::Status(405, _)) => Err(
            "This endpoint does not list its models — type the model name yourself.".to_string(),
        ),
        Err(ureq::Error::Status(status, response)) => {
            let body = response.into_string().unwrap_or_default();
            Err(error_detail(status, &body))
        }
        Err(ureq::Error::Transport(transport)) => Err(format!(
            "Could not reach the endpoint. Check the address and your connection. ({transport})"
        )),
    }
}

pub(crate) fn error_detail(status: u16, body: &str) -> String {
    let parsed = serde_json::from_str::<Value>(body).ok();
    let message = parsed
        .as_ref()
        .and_then(|v| v["error"]["message"].as_str())
        .or_else(|| parsed.as_ref().and_then(|v| v["message"].as_str()))
        .unwrap_or(body);
    format!("HTTP {status}: {}", trim_chars(message.trim(), 300))
}

pub fn test_endpoint(settings: &AiSettings) -> Result<String, String> {
    let base = settings.base_url.trim();
    if base.is_empty() {
        return Err("Add an endpoint address first.".to_string());
    }
    if settings.is_demo() {
        return Err(
            "The demo writer is active. Add an API key, or point the address at a machine on your own network."
                .to_string(),
        );
    }
    let url = models_endpoint(base);
    let mut request = ureq::get(&url).timeout(Duration::from_secs(20));
    if !settings.api_key.trim().is_empty() {
        request = request.set(
            "Authorization",
            &format!("Bearer {}", settings.api_key.trim()),
        );
    }
    match request.call() {
        Ok(response) => {
            let status = response.status();
            let text = response.into_string().unwrap_or_default();
            let count = parse_models(&text).ok().map(|names| names.len());
            Ok(match count {
                Some(1) => format!("Connected (HTTP {status}). 1 model is listed."),
                Some(n) => format!("Connected (HTTP {status}). {n} models are listed."),
                None => format!("Connected (HTTP {status})."),
            })
        }
        Err(ureq::Error::Status(404, _)) | Err(ureq::Error::Status(405, _)) => {
            // Many local servers do not implement /models; try the actual chat route.
            probe_chat(settings)
        }
        Err(ureq::Error::Status(status, response)) => {
            let body = response.into_string().unwrap_or_default();
            Err(error_detail(status, &body))
        }
        Err(ureq::Error::Transport(transport)) => {
            Err(format!("Could not reach the endpoint. ({transport})"))
        }
    }
}

pub(crate) fn probe_chat(settings: &AiSettings) -> Result<String, String> {
    let url = endpoint(&settings.base_url);
    let body = json!({
        "model": settings.model,
        "messages": [{"role": "user", "content": "Reply with the single word: ready"}],
        "max_tokens": 8,
        "temperature": 0.0
    });
    let mut request = ureq::post(&url)
        .set("Content-Type", "application/json")
        .timeout(Duration::from_secs(30));
    if !settings.api_key.trim().is_empty() {
        request = request.set(
            "Authorization",
            &format!("Bearer {}", settings.api_key.trim()),
        );
    }
    match request.send_json(body) {
        Ok(response) => {
            let status = response.status();
            Ok(format!("Connected (HTTP {status}). The model answered."))
        }
        Err(ureq::Error::Status(status, response)) => {
            let body = response.into_string().unwrap_or_default();
            Err(error_detail(status, &body))
        }
        Err(ureq::Error::Transport(transport)) => {
            Err(format!("Could not reach the endpoint. ({transport})"))
        }
    }
}
