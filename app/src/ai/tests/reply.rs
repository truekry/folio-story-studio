//! Seams, endings and continuation across the length limit.

use super::*;

#[test]
fn a_continued_reply_drops_the_words_it_repeats() {
    fn joined(before: &str, after: &str) -> String {
        format!("{before}{}", continuation_suffix(before, after))
            .trim_end()
            .to_string()
    }
    // The model repeats the word it stopped on.
    assert_eq!(
        joined("Mira opened the", "the door and went in."),
        "Mira opened the door and went in."
    );
    // It repeats a whole sentence.
    assert_eq!(
        joined(
            "The bell rang. Mira turned.",
            "The bell rang. Mira turned. The hall was empty."
        ),
        "The bell rang. Mira turned. The hall was empty."
    );
    // It was cut in the middle of a word.
    assert_eq!(joined("The do", "or opened."), "The door opened.");
    // It continues cleanly and repeats nothing.
    assert_eq!(
        joined("The door opened.", "Mira stepped through."),
        "The door opened. Mira stepped through."
    );
    // The first text already ends with the space the next one needs.
    assert_eq!(
        joined("The door opened. ", "Mira stepped through."),
        "The door opened. Mira stepped through."
    );
    // An empty continuation changes nothing.
    assert_eq!(continuation_suffix("Done.", "  "), "");
}

#[test]
fn a_summary_never_ends_in_the_middle_of_a_sentence() {
    let (text, cut) = complete_summary("Mira finds the door sealed. She takes the key and");
    assert_eq!(text, "Mira finds the door sealed.");
    assert!(cut, "the caller wants to know that a tail was dropped");

    let (text, cut) = complete_summary("Mira finds the door sealed.");
    assert_eq!(text, "Mira finds the door sealed.");
    assert!(!cut);

    // A summary that ends inside a quote is finished too.
    let (_, cut) = complete_summary("She says only one word: \"Tomorrow.\"");
    assert!(!cut);

    // Without any full stop there is nothing to trim to.
    let (text, cut) = complete_summary("A fragment without punctuation");
    assert_eq!(text, "A fragment without punctuation");
    assert!(!cut);
}

#[test]
fn a_cut_reply_is_continued_until_the_sentence_ends() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let address = listener.local_addr().expect("mock address");
    let worker = std::thread::spawn(move || {
        let mut seen: Vec<Value> = Vec::new();
        for round in 0..2 {
            let (mut stream, _) = listener.accept().expect("accept request");
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
            seen.push(serde_json::from_slice(&body).expect("request json"));
            let (content, reason) = if round == 0 {
                ("Mira opened the", "length")
            } else {
                ("the door and went in.", "stop")
            };
            let payload = json!({
                "choices": [{
                    "message": {"role": "assistant", "content": content},
                    "finish_reason": reason
                }]
            })
            .to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                payload.len(),
                payload
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
        seen
    });
    let settings = AiSettings {
        base_url: format!("http://{address}/v1"),
        api_key: "test-key".to_string(),
        model: "mock".to_string(),
        // Close enough to the prompt that the second round has to share the
        // remaining window instead of asking for the first round's room again.
        context_limit: 600,
        ..Default::default()
    };
    let reply = request_with_target(&settings, "Summarize chapter two.", &[], "", 90)
        .expect("continued reply");
    assert_eq!(reply, "Mira opened the door and went in.");

    let requests = worker.join().expect("mock server finished");
    assert_eq!(requests.len(), 2, "the cut reply must be continued once");
    // The first request carries the word target as reply room.
    assert_eq!(requests[0]["max_tokens"], 380);
    // The partial text and the continue note now fill part of the window, so
    // the second round may ask for less. Asking for the full 380 again would
    // push prompt and answer past the endpoint's 600-token window.
    let second_cap = requests[1]["max_tokens"].as_u64().expect("second cap");
    assert!(second_cap < 380, "second cap was {second_cap}");
    assert!(second_cap >= 64, "second cap was {second_cap}");
    // The second one shows the partial text and asks to go on.
    let messages = requests[1]["messages"].as_array().expect("messages");
    let last = messages.last().expect("last message");
    assert_eq!(last["role"], "user");
    assert!(
        last["content"]
            .as_str()
            .unwrap_or_default()
            .contains("Continue exactly where you stopped"),
        "the follow-up must say what to do"
    );
    assert_eq!(
        messages[messages.len() - 2]["content"],
        "Mira opened the",
        "the partial text travels as the assistant turn"
    );
}
