//! Reading a reply as it arrives: the SSE reader, the retry across the length limit and the cancel flag.

use super::*;

/// A long reply may take minutes; only a silent socket is an error. The read
/// timeout applies to each read, not to the whole reply. It is generous because
/// a local model can spend a long time on the prompt before its first token.
pub(crate) const STREAM_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

pub(crate) const STREAM_READ_TIMEOUT: Duration = Duration::from_secs(90);

/// One payload from a server-sent event stream.
#[derive(Debug, PartialEq)]
pub(crate) enum SseFrame {
    Data(String),
    Done,
}

/// Turns the lines of an SSE response into frames. JSON payloads may arrive
/// split over several `data:` lines; `[DONE]` closes the stream. A server that
/// forgets the blank line between frames is handled too: a payload that already
/// parses as JSON is handed over before the next line is added.
#[derive(Default)]
pub(crate) struct SseLines {
    pending: String,
}

impl SseLines {
    pub(crate) fn push(&mut self, line: &str) -> Vec<SseFrame> {
        let line = line.trim_end_matches(|c| c == '\r' || c == '\n');
        if line.is_empty() {
            return self.take();
        }
        let Some(rest) = line.strip_prefix("data:") else {
            // `event:`, `id:` and comments carry nothing Folio needs.
            return Vec::new();
        };
        let rest = rest.strip_prefix(' ').unwrap_or(rest);
        let mut frames = Vec::new();
        if !self.pending.is_empty() && payload_is_complete(&self.pending) {
            frames.extend(self.take());
        }
        if !self.pending.is_empty() {
            self.pending.push('\n');
        }
        self.pending.push_str(rest);
        if payload_is_complete(&self.pending) {
            frames.extend(self.take());
        }
        frames
    }

    /// The last frame need not be followed by a blank line.
    pub(crate) fn finish(&mut self) -> Vec<SseFrame> {
        self.take()
    }

    fn take(&mut self) -> Vec<SseFrame> {
        let pending = std::mem::take(&mut self.pending);
        let pending = pending.trim();
        if pending.is_empty() {
            Vec::new()
        } else if pending == "[DONE]" {
            vec![SseFrame::Done]
        } else {
            vec![SseFrame::Data(pending.to_string())]
        }
    }
}

pub(crate) fn payload_is_complete(payload: &str) -> bool {
    let payload = payload.trim();
    payload == "[DONE]" || serde_json::from_str::<Value>(payload).is_ok()
}

/// The text of one delta frame. OpenAI-compatible servers use `delta.content`;
/// a few use `text`, and some only send a final `message.content`.
pub(crate) fn delta_text(value: &Value) -> Option<&str> {
    let choice = &value["choices"][0];
    choice["delta"]["content"]
        .as_str()
        .or_else(|| choice["text"].as_str())
}

/// An error that arrives inside the stream instead of as an HTTP status.
pub(crate) fn stream_error(value: &Value) -> Option<String> {
    let error = &value["error"];
    if error.is_null() {
        return None;
    }
    Some(
        error["message"]
            .as_str()
            .or_else(|| error.as_str())
            .unwrap_or("the endpoint reported an error")
            .to_string(),
    )
}

pub(crate) fn apply_stream_value(
    value: &Value,
    text: &mut String,
    truncated: &mut bool,
    on_delta: &mut dyn FnMut(&str),
) -> Result<(), String> {
    if let Some(error) = stream_error(value) {
        return Err(format!("The endpoint stopped the reply: {error}"));
    }
    if let Some(delta) = delta_text(value) {
        if !delta.is_empty() {
            text.push_str(delta);
            on_delta(delta);
        }
    }
    if was_truncated(value) {
        *truncated = true;
    }
    Ok(())
}

pub(crate) fn apply_sse_frames(
    frames: Vec<SseFrame>,
    text: &mut String,
    truncated: &mut bool,
    last: &mut Option<Value>,
    done: &mut bool,
    on_delta: &mut dyn FnMut(&str),
) -> Result<(), String> {
    for frame in frames {
        match frame {
            SseFrame::Done => *done = true,
            SseFrame::Data(payload) => {
                // A keep-alive or an unknown dialect is skipped rather than
                // ending a reply that is otherwise fine.
                let Ok(value) = serde_json::from_str::<Value>(&payload) else {
                    continue;
                };
                apply_stream_value(&value, text, truncated, on_delta)?;
                *last = Some(value);
            }
        }
    }
    Ok(())
}

/// One streaming request. Each new piece of text is handed to `on_delta` as it
/// arrives; the finished reply is returned once the server closes the stream.
/// A server that ignores `"stream": true` and answers with one JSON body is
/// accepted as a single delta.
pub(crate) fn stream_once(
    settings: &AiSettings,
    messages: &[Value],
    max_tokens: u32,
    cancel: &AtomicBool,
    on_delta: &mut dyn FnMut(&str),
) -> Result<Reply, String> {
    let url = endpoint(&settings.base_url);
    let body = json!({
        "model": settings.model,
        "messages": messages,
        "temperature": settings.temperature,
        "max_tokens": max_tokens,
        "stream": true
    });
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(STREAM_CONNECT_TIMEOUT)
        .timeout_read(STREAM_READ_TIMEOUT)
        .build();
    let mut request = agent.post(&url).set("Content-Type", "application/json");
    if !settings.api_key.trim().is_empty() {
        request = request.set(
            "Authorization",
            &format!("Bearer {}", settings.api_key.trim()),
        );
    }
    let response = match request.send_json(body) {
        Ok(response) => response,
        Err(ureq::Error::Status(status, response)) => {
            let body = response.into_string().unwrap_or_default();
            return Err(error_detail(status, &body));
        }
        Err(ureq::Error::Transport(transport)) => {
            return Err(format!(
                "Could not reach the endpoint. Check the address and your connection. ({transport})"
            ))
        }
    };
    let status = response.status();
    if !(200..300).contains(&status) {
        let body = response.into_string().unwrap_or_default();
        return Err(error_detail(status, &body));
    }

    let content_type = response
        .header("content-type")
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !content_type.contains("text/event-stream") {
        let body = response
            .into_string()
            .map_err(|e| format!("Could not read the reply: {e}"))?;
        let value: Value = serde_json::from_str(&body).map_err(|_| {
            format!(
                "The endpoint sent something that is not JSON: {}",
                trim_chars(&body, 240)
            )
        })?;
        let reply = extract_content(&value)
            .ok_or_else(|| format!("The reply had no text: {}", trim_chars(&body, 240)))?;
        if reply.trim().is_empty() {
            return Err("The reply had no text.".to_string());
        }
        on_delta(&reply);
        return Ok(Reply {
            text: reply,
            truncated: was_truncated(&value),
        });
    }

    let reader = BufReader::new(response.into_reader());
    let mut lines = SseLines::default();
    let mut text = String::new();
    let mut truncated = false;
    let mut last: Option<Value> = None;
    let mut done = false;
    for line in reader.lines() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let line = line.map_err(|e| format!("Could not read the reply: {e}"))?;
        let frames = lines.push(&line);
        apply_sse_frames(
            frames,
            &mut text,
            &mut truncated,
            &mut last,
            &mut done,
            on_delta,
        )?;
        if done {
            break;
        }
    }
    if !done {
        let frames = lines.finish();
        apply_sse_frames(
            frames,
            &mut text,
            &mut truncated,
            &mut last,
            &mut done,
            on_delta,
        )?;
    }

    // Some servers send the whole answer as `message.content` in the final
    // frame. That is not streamed, but it is still a usable reply.
    if text.trim().is_empty() {
        if let Some(value) = last.as_ref() {
            if let Some(content) = extract_content(value) {
                if !content.trim().is_empty() {
                    text = content;
                    on_delta(&text);
                    if was_truncated(value) {
                        truncated = true;
                    }
                }
            }
        }
    }
    if text.trim().is_empty() && !cancel.load(Ordering::Relaxed) {
        return Err("The reply had no text.".to_string());
    }
    Ok(Reply { text, truncated })
}

/// The streaming twin of `request_with_target`: the first round reaches the UI
/// piece by piece, later rounds (after a token limit) are stitched on and
/// announced with one delta, so nothing appears twice. `on_delta` must append
/// to the same text the caller already holds.
pub fn stream_with_target(
    settings: &AiSettings,
    prompt: &str,
    history: &[ChatMessage],
    memory: &str,
    system: Option<&str>,
    target_words: u32,
    allow_continuation: bool,
    cancel: &AtomicBool,
    on_delta: &mut dyn FnMut(&str),
) -> Result<Reply, String> {
    if settings.is_demo() {
        return Err("No endpoint is configured.".to_string());
    }
    let wanted = reply_token_wish(settings, target_words);
    let mut room = reply_room(settings, prompt, history, memory);
    let mut cap = reply_budget(settings, prompt, history, memory, wanted)?;
    let mut messages = messages_for(prompt, history, memory, system);
    let mut text = String::new();
    let mut rounds = 0;
    loop {
        rounds += 1;
        let reply = if rounds == 1 {
            stream_once(settings, &messages, cap, cancel, on_delta)?
        } else {
            // The seam between two rounds needs `continuation_suffix`, so this
            // round is collected first and sent as one piece at the end.
            stream_once(settings, &messages, cap, cancel, &mut |_: &str| {})?
        };
        if rounds == 1 {
            text = reply.text;
        } else {
            let suffix = continuation_suffix(&text, &reply.text);
            if !suffix.is_empty() {
                on_delta(&suffix);
            }
            text.push_str(&suffix);
        }
        let unfinished =
            reply.truncated || (target_words > 0 && rounds == 1 && !ends_sentence(&text));
        if cancel.load(Ordering::Relaxed) {
            return Ok(Reply {
                text,
                truncated: reply.truncated,
            });
        }
        if !allow_continuation || target_words == 0 || !unfinished || rounds >= 3 {
            return Ok(Reply {
                text,
                truncated: unfinished,
            });
        }
        // The next round carries everything written so far and the continue
        // note. Both take room from the same window, so the follow-up may ask
        // for less than the first round did. Without this, a chapter already
        // close to the context limit would overflow it on the second round.
        if let Some(left) = room.as_mut() {
            let added = estimated_tokens(&text) + estimated_tokens(CONTINUE_AFTER_LIMIT);
            *left = left.saturating_sub(added);
            if *left < 64 {
                return Ok(Reply {
                    text,
                    truncated: true,
                });
            }
            cap = cap.min(*left as u32).max(64);
        }
        messages.push(json!({"role": "assistant", "content": text.clone()}));
        messages.push(json!({"role": "user", "content": CONTINUE_AFTER_LIMIT}));
    }
}
