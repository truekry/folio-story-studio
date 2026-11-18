//! Reading an answer frame by frame from a small local server.

use super::*;

fn read_http_json(stream: &mut std::net::TcpStream) -> Value {
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
    serde_json::from_slice(&body).expect("request json")
}

fn start_sse(stream: &mut std::net::TcpStream) {
    let _ = stream.write_all(
        b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n",
    );
    let _ = stream.flush();
}

fn send_sse(stream: &mut std::net::TcpStream, payload: &str) {
    let _ = write!(stream, "data: {payload}\n\n");
    let _ = stream.flush();
}

#[test]
fn sse_lines_become_frames() {
    let mut lines = SseLines::default();
    // A payload split over two data lines is one frame.
    assert!(lines.push("data: {\"choices\":").is_empty());
    assert_eq!(
        lines.push("data: []}"),
        vec![SseFrame::Data("{\"choices\":\n[]}".to_string())]
    );
    // A complete payload needs no blank line after it.
    assert_eq!(
        lines.push("data: {\"x\":1}\r"),
        vec![SseFrame::Data("{\"x\":1}".to_string())]
    );
    // Comments and event names carry nothing.
    assert!(lines.push(": keep-alive").is_empty());
    assert!(lines.push("event: message").is_empty());
    assert_eq!(lines.push("data: [DONE]"), vec![SseFrame::Done]);

    // A final frame without a blank line is flushed at the end.
    let mut tail = SseLines::default();
    assert!(tail.push("data: not json").is_empty());
    assert_eq!(tail.finish(), vec![SseFrame::Data("not json".to_string())]);
}

#[test]
fn a_streamed_reply_arrives_piece_by_piece() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let address = listener.local_addr().expect("mock address");
    let worker = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        let request = read_http_json(&mut stream);
        assert_eq!(request["stream"], true, "the request must ask for a stream");
        start_sse(&mut stream);
        for piece in ["The ", "bell ", "answered."] {
            send_sse(
                &mut stream,
                &json!({"choices": [{"delta": {"content": piece}}]}).to_string(),
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        send_sse(
            &mut stream,
            &json!({"choices": [{"delta": {}, "finish_reason": "stop"}]}).to_string(),
        );
        send_sse(&mut stream, "[DONE]");
    });
    let settings = AiSettings {
        base_url: format!("http://{address}/v1"),
        api_key: "test-key".to_string(),
        model: "mock".to_string(),
        ..Default::default()
    };
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let mut deltas: Vec<String> = Vec::new();
    let reply = stream_with_target(
        &settings,
        "Continue the scene.",
        &[],
        "",
        None,
        0,
        true,
        &cancel,
        &mut |delta| deltas.push(delta.to_string()),
    )
    .expect("streamed reply");

    assert_eq!(deltas, vec!["The ", "bell ", "answered."]);
    assert_eq!(reply.text, "The bell answered.");
    assert!(!reply.truncated);
    worker.join().expect("mock server finished");
}

#[test]
fn a_server_that_ignores_stream_still_answers() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let address = listener.local_addr().expect("mock address");
    let worker = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        let request = read_http_json(&mut stream);
        assert_eq!(request["stream"], true);
        let body = r#"{"choices":[{"message":{"role":"assistant","content":"The bell answered."},"finish_reason":"stop"}]}"#;
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
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let mut deltas: Vec<String> = Vec::new();
    let reply = stream_with_target(
        &settings,
        "Continue the scene.",
        &[],
        "",
        None,
        0,
        true,
        &cancel,
        &mut |delta| deltas.push(delta.to_string()),
    )
    .expect("plain reply");

    assert_eq!(deltas, vec!["The bell answered."]);
    assert_eq!(reply.text, "The bell answered.");
    worker.join().expect("mock server finished");
}

#[test]
fn a_cut_stream_is_continued_without_repeating_the_seam() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let address = listener.local_addr().expect("mock address");
    let worker = std::thread::spawn(move || {
        let mut requests = Vec::new();
        for round in 0..2 {
            let (mut stream, _) = listener.accept().expect("accept request");
            requests.push(read_http_json(&mut stream));
            start_sse(&mut stream);
            let (piece, reason) = if round == 0 {
                ("Mira opened the", "length")
            } else {
                ("the door and went in.", "stop")
            };
            send_sse(
                &mut stream,
                &json!({"choices": [{"delta": {"content": piece}, "finish_reason": reason}]})
                    .to_string(),
            );
            send_sse(&mut stream, "[DONE]");
        }
        requests
    });
    let settings = AiSettings {
        base_url: format!("http://{address}/v1"),
        api_key: "test-key".to_string(),
        model: "mock".to_string(),
        ..Default::default()
    };
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let mut deltas: Vec<String> = Vec::new();
    let reply = stream_with_target(
        &settings,
        "Summarize chapter two.",
        &[],
        "",
        None,
        90,
        true,
        &cancel,
        &mut |delta| deltas.push(delta.to_string()),
    )
    .expect("continued stream");

    assert_eq!(reply.text, "Mira opened the door and went in.");
    assert_eq!(
        deltas.concat(),
        reply.text,
        "the deltas must add up to the finished text"
    );

    let requests = worker.join().expect("mock server finished");
    assert_eq!(requests.len(), 2, "the cut stream must be continued once");
    assert_eq!(requests[0]["max_tokens"], 280);
    let messages = requests[1]["messages"].as_array().expect("messages");
    let last = messages.last().expect("last message");
    assert!(
        last["content"]
            .as_str()
            .unwrap_or_default()
            .contains("Continue exactly where you stopped"),
        "the follow-up must say what to do"
    );
}

#[test]
fn a_summary_is_not_continued_into_a_second_account() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let address = listener.local_addr().expect("mock address");
    let worker = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("one request");
        let request = read_http_json(&mut stream);
        start_sse(&mut stream);
        send_sse(
            &mut stream,
            &json!({"choices": [{"delta": {"content": "It was a normal sunny day."}, "finish_reason": "length"}]})
                .to_string(),
        );
        send_sse(&mut stream, "[DONE]");
        request
    });
    let settings = AiSettings {
        base_url: format!("http://{address}/v1"),
        api_key: "test-key".to_string(),
        model: "mock".to_string(),
        ..Default::default()
    };
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let mut deltas: Vec<String> = Vec::new();
    let reply = stream_with_target(
        &settings,
        "Summarize the new chapter.",
        &[],
        "",
        None,
        200,
        false,
        &cancel,
        &mut |delta| deltas.push(delta.to_string()),
    )
    .expect("summary stream");

    assert_eq!(reply.text, "It was a normal sunny day.");
    assert!(
        reply.truncated,
        "the caller still needs to know the reply was cut"
    );
    // Only one request: a summary must never be stitched onto a second
    // account, which would store the same events twice.
    let _request = worker.join().expect("mock server finished");
    assert_eq!(deltas.concat(), reply.text);
}

#[test]
fn a_stream_can_be_stopped_between_frames() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let address = listener.local_addr().expect("mock address");
    let worker = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        let _ = read_http_json(&mut stream);
        start_sse(&mut stream);
        for piece in ["First.", "Second.", "Third."] {
            send_sse(
                &mut stream,
                &json!({"choices": [{"delta": {"content": piece}}]}).to_string(),
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        send_sse(&mut stream, "[DONE]");
    });
    let settings = AiSettings {
        base_url: format!("http://{address}/v1"),
        api_key: "test-key".to_string(),
        model: "mock".to_string(),
        ..Default::default()
    };
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let mut deltas: Vec<String> = Vec::new();
    let reply = stream_with_target(
        &settings,
        "Continue the scene.",
        &[],
        "",
        None,
        0,
        true,
        &cancel,
        &mut |delta| {
            deltas.push(delta.to_string());
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        },
    )
    .expect("stopped stream");

    assert_eq!(deltas, vec!["First."]);
    assert_eq!(reply.text, "First.");
    worker.join().expect("mock server finished");
}

#[test]
fn an_error_frame_keeps_the_text_that_arrived() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let address = listener.local_addr().expect("mock address");
    let worker = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        let _ = read_http_json(&mut stream);
        start_sse(&mut stream);
        send_sse(
            &mut stream,
            &json!({"choices": [{"delta": {"content": "First."}}]}).to_string(),
        );
        send_sse(
            &mut stream,
            &json!({"error": {"message": "mock stream failure"}}).to_string(),
        );
    });
    let settings = AiSettings {
        base_url: format!("http://{address}/v1"),
        api_key: "test-key".to_string(),
        model: "mock".to_string(),
        ..Default::default()
    };
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let mut deltas: Vec<String> = Vec::new();
    let error = stream_with_target(
        &settings,
        "Continue the scene.",
        &[],
        "",
        None,
        0,
        true,
        &cancel,
        &mut |delta| deltas.push(delta.to_string()),
    )
    .expect_err("the error frame must end the request");

    assert!(error.contains("mock stream failure"), "{error}");
    assert_eq!(deltas, vec!["First."], "the caller keeps what arrived");
    worker.join().expect("mock server finished");
}
