//! Integration tests against a real OpenAI-compatible endpoint.
//!
//! The tests are marked `#[ignore]`, so the normal run stays offline and
//! deterministic. To run them, point `FOLIO_LAN_URL` at any endpoint – a local
//! KoboldCpp, LM Studio, Ollama or a remote service – and include the ignored
//! tests:
//!
//! ```text
//! FOLIO_LAN_URL=http://localhost:1234/v1 cargo test -- --include-ignored --test-threads=1
//! ```
//!
//! `FOLIO_LAN_MODEL` and `FOLIO_LAN_API_KEY` are optional. Without a model name
//! the endpoint's loaded model answers.

use super::*;

/// The endpoint from the environment. A missing `FOLIO_LAN_URL` panics with the
/// one line that says how to set it.
fn lan_settings() -> AiSettings {
    let Ok(base_url) = std::env::var("FOLIO_LAN_URL") else {
        panic!(
            "set FOLIO_LAN_URL to an OpenAI-compatible endpoint, \
             for example http://localhost:1234/v1"
        );
    };
    AiSettings {
        base_url,
        api_key: std::env::var("FOLIO_LAN_API_KEY").unwrap_or_default(),
        model: std::env::var("FOLIO_LAN_MODEL").unwrap_or_default(),
        ..Default::default()
    }
}

/// The model list, the context window it reports, and one short answer.
#[test]
#[ignore = "needs FOLIO_LAN_URL"]
fn the_lan_endpoint_reports_its_window() {
    let settings = lan_settings();
    let models = list_models(&settings).expect("models from the endpoint");
    assert!(
        !models.names.is_empty(),
        "the endpoint should list at least one model"
    );
    if let Some(limit) = models.context_limit {
        assert!(limit > 0, "a reported context window must be positive");
    }
    let reply =
        request_with_target(&settings, "Answer with one word: ready?", &[], "", 0).expect("reply");
    assert!(!reply.trim().is_empty());
}

/// The lamp's own connection check against the endpoint.
#[test]
#[ignore = "needs FOLIO_LAN_URL"]
fn the_lan_lamp_finds_the_endpoint_and_its_model() {
    let settings = lan_settings();
    let message = test_endpoint(&settings).expect("the lamp's connection check");
    assert!(message.contains("Connected (HTTP"), "{message}");
    assert!(message.contains("model"), "{message}");
    let models = list_models(&settings).expect("models from the endpoint");
    assert!(!models.names.is_empty());
    if let Some(limit) = models.context_limit {
        assert!(limit > 0, "the endpoint should report a usable window");
    }
}

/// A steering note has to reach the real model and be followed.
#[test]
#[ignore = "needs FOLIO_LAN_URL"]
fn steering_on_the_lan_endpoint_shapes_the_reply() {
    let settings = lan_settings();
    let library = Library::sample();
    let story = &library.stories[0];
    let mut chapter = story.chapters[1].clone();
    chapter.steering =
        "Mira finds a brass key inside the teacup, and the storm breaks over the harbour."
            .to_string();
    let prompt = chat_prompt(story, &chapter, Action::Continue, "", "", &settings);
    assert!(
        prompt.contains("STEERING"),
        "the note must reach the prompt"
    );
    let mut reply = request_with_target(&settings, &prompt, &[], "", 120).expect("reply");
    // A local model occasionally falls into a special-token loop. Ask once
    // more before treating that as a steering failure.
    if reply.split_whitespace().count() <= 20
        || !(reply.to_lowercase().contains("key") || reply.to_lowercase().contains("storm"))
    {
        reply = request_with_target(&settings, &prompt, &[], "", 120).expect("retry reply");
    }
    let lower = reply.to_lowercase();
    assert!(
        reply.split_whitespace().count() > 20,
        "the reply was too short: {reply}"
    );
    assert!(
        lower.contains("key") || lower.contains("storm"),
        "the reply ignored the note: {reply}"
    );
}

/// The lorebook pass against the real model.
#[test]
#[ignore = "needs FOLIO_LAN_URL"]
fn the_lan_lore_pass_proposes_a_fitting_entry() {
    let mut settings = lan_settings();
    // The app lowers the temperature for a lore pass so the model records
    // facts instead of inventing prose; the test has to do the same.
    settings.temperature = 0.3;
    let library = Library::sample();
    let story = &library.stories[0];
    let passage = "At the end of the pier, Wren Okafor lit the second lantern. It burned green, the way the old maps said the Hollow Lanterns burn when someone is being remembered.";
    let prompt = lore_prompt(story, passage);
    assert!(prompt.contains("Wren Okafor"));
    let reply = request_with_target(&settings, &prompt, &[], "", 0).expect("reply");
    let suggestion = parse_lore_reply(&reply, story);
    assert!(
        suggestion.well_formed,
        "the model ignored the labels: {reply}"
    );
    assert!(!suggestion.text.trim().is_empty(), "no facts: {reply}");
    assert!(
        suggestion.target.is_some() || !suggestion.name.trim().is_empty(),
        "the model should have named an entry: {reply}"
    );
}

/// The proofreader against the real model.
#[test]
#[ignore = "needs FOLIO_LAN_URL"]
fn the_lan_proofreader_corrects_a_paragraph() {
    let settings = lan_settings();
    let cancel = AtomicBool::new(false);
    let paragraph = "She walk to the harbour every morning, and the see was quiet.";
    let reply = proofread_paragraph(&settings, paragraph, &cancel).expect("reply");
    let lower = reply.to_lowercase();
    assert!(
        lower.contains("walks") || lower.contains("walked"),
        "the verb was not corrected: {reply}"
    );
    assert!(
        lower.contains("sea"),
        "the misspelling was not corrected: {reply}"
    );
    assert!(
        !lower.contains("proofread") && !lower.contains("corrected paragraph"),
        "the reply must be only the paragraph: {reply}"
    );
}

/// The block-wise pipeline against the real model: every block is summarized
/// in turn, then the combined summary is rewritten as one text.
#[test]
#[ignore = "needs FOLIO_LAN_URL"]
fn the_lan_block_summary_pipeline_returns_one_text() {
    let mut settings = lan_settings();
    settings.temperature = 0.3;
    let library = Library::sample();
    let story = &library.stories[0];
    let chapter = &story.chapters[0];
    let mut body = String::new();
    for part in story.chapters.iter().take(3) {
        body.push_str(part.body.trim());
        body.push_str("\n\n");
    }
    body.push_str(chapter.body.trim());
    body.push_str("\n\n");
    let blocks = summary_blocks(&body, SUMMARY_BLOCK_WORDS);
    assert!(
        blocks.len() > 1,
        "the fixture must need more than one block"
    );

    let cancel = AtomicBool::new(false);
    let mut parts = Vec::new();
    for (index, range) in blocks.iter().enumerate() {
        let prompt = summary_block_prompt(
            story,
            chapter,
            &body[range.clone()],
            index,
            blocks.len(),
            &settings,
        );
        let mut reply = stream_with_target(
            &settings,
            &prompt,
            &[],
            "",
            None,
            summary_block_words(&settings),
            true,
            &cancel,
            &mut |_: &str| {},
        );
        if reply
            .as_ref()
            .map(|reply| reply.text.trim().is_empty())
            .unwrap_or(true)
        {
            reply = stream_with_target(
                &settings,
                &prompt,
                &[],
                "",
                None,
                summary_block_words(&settings),
                true,
                &cancel,
                &mut |_: &str| {},
            );
        }
        let reply = reply.expect("block summary");
        let (text, _) = complete_summary(&reply.text);
        assert!(!text.trim().is_empty(), "block {index} had no text");
        parts.push(text);
    }

    let combined = parts.join("\n\n");
    let prompt = summary_merge_prompt(&combined);
    let words = parts
        .iter()
        .map(|part| part.split_whitespace().count())
        .sum::<usize>() as u32;
    let mut reply = stream_with_target(
        &settings,
        &prompt,
        &[],
        "",
        None,
        words,
        true,
        &cancel,
        &mut |_: &str| {},
    );
    if reply
        .as_ref()
        .map(|reply| reply.text.trim().is_empty())
        .unwrap_or(true)
    {
        reply = stream_with_target(
            &settings,
            &prompt,
            &[],
            "",
            None,
            words,
            true,
            &cancel,
            &mut |_: &str| {},
        );
    }
    let reply = reply.expect("merge reply");
    let (text, _) = complete_summary(&reply.text);
    // This test checks the plumbing: every block gets its own answer and the
    // merge sees the combined text. The wording itself is the model's job and
    // is deliberately not asserted here.
    assert!(!text.trim().is_empty(), "the merged summary was empty");
}

#[test]
fn mock_endpoint_round_trip() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let address = listener.local_addr().expect("mock address");
    let worker = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        // Consume the whole request before answering. Closing a Windows socket
        // with unread request bytes can reset the connection and lose the reply.
        let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
        let mut request_line = String::new();
        reader.read_line(&mut request_line).expect("request line");
        let mut content_length = 0usize;
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap_or(0) == 0 {
                break;
            }
            if line == "\r\n" || line == "\n" {
                break;
            }
            if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                content_length = value.trim().parse().unwrap_or(0);
            }
        }
        let mut body = vec![0u8; content_length];
        let _ = reader.read_exact(&mut body);
        assert!(request_line.contains("POST /v1/chat/completions"));
        let request: Value = serde_json::from_slice(&body).expect("request json");
        let messages = request["messages"].as_array().expect("messages");
        assert_eq!(messages.len(), 4, "system, two turns, prompt");
        assert_eq!(messages[0]["role"], "system");
        assert!(
            messages[0]["content"]
                .as_str()
                .unwrap_or_default()
                .contains("EARLIER IN THIS CONVERSATION"),
            "the condensed memory must reach the system prompt"
        );
        assert_eq!(messages[1]["role"], "user");
        assert_eq!(messages[1]["content"], "What is behind the wall?");
        assert_eq!(messages[2]["role"], "assistant");
        assert_eq!(messages[2]["content"], "A corridor of warm stones.");
        assert_eq!(messages[3]["role"], "user");
        assert!(
            messages[3]["content"]
                .as_str()
                .unwrap_or_default()
                .contains("Continue the scene."),
            "the prompt travels as the last user turn"
        );
        let body =
            r#"{"choices":[{"message":{"role":"assistant","content":"The bell answered."}}]}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
    });
    let settings = AiSettings {
        base_url: format!("http://{address}/v1"),
        api_key: "test-key".to_string(),
        model: "mock".to_string(),
        ..Default::default()
    };
    let history = vec![
        ChatMessage {
            role: "user".to_string(),
            text: "What is behind the wall?".to_string(),
            demo: false,
        },
        ChatMessage {
            role: "assistant".to_string(),
            text: "A corridor of warm stones.".to_string(),
            demo: false,
        },
    ];
    let reply = request_with_target(
        &settings,
        "Continue the scene.",
        &history,
        "The bell answered twice.",
        0,
    )
    .expect("mock reply");
    assert_eq!(reply, "The bell answered.");
    worker.join().expect("mock server finished");
}
