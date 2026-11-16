//! Addresses, model lists and context windows.

use super::*;

#[test]
fn a_bare_address_gets_a_scheme() {
    assert_eq!(
        endpoint("192.168.10.20:5001/v1"),
        "http://192.168.10.20:5001/v1/chat/completions"
    );
    assert_eq!(
        models_endpoint("192.168.10.20:5001/v1"),
        "http://192.168.10.20:5001/v1/models"
    );
    // A pasted full chat route still reaches the catalogue next to it.
    assert_eq!(
        models_endpoint("192.168.10.20:5001/v1/chat/completions"),
        "http://192.168.10.20:5001/v1/models"
    );
    assert_eq!(
        models_endpoint("https://api.openai.com/v1/chat/completions/"),
        "https://api.openai.com/v1/models"
    );
    // A public name is assumed to speak HTTPS, not HTTP.
    assert_eq!(
        endpoint("api.openai.com/v1"),
        "https://api.openai.com/v1/chat/completions"
    );
    assert_eq!(
        endpoint("https://api.openai.com/v1/"),
        "https://api.openai.com/v1/chat/completions"
    );
    // A leftover https:// from a preset meets a local server with plain
    // HTTP — otherwise the TLS handshake fails as "corrupt message".
    assert_eq!(
        endpoint("https://192.168.10.20:5001/v1"),
        "http://192.168.10.20:5001/v1/chat/completions"
    );
    assert_eq!(
        endpoint("https://localhost:8080/v1"),
        "http://localhost:8080/v1/chat/completions"
    );
    // The server root keeps the scheme rule too.
    assert_eq!(origin("192.168.10.20:5001/v1"), "http://192.168.10.20:5001");
    assert_eq!(
        origin("https://api.openai.com/v1"),
        "https://api.openai.com"
    );
}

#[test]
fn a_context_window_is_read_from_common_shapes() {
    // KoboldCpp answers with a bare number.
    assert_eq!(context_from_text("8192"), Some(8192));
    assert_eq!(context_from_text(" 4096\n"), Some(4096));
    // llama.cpp /props keeps it under default_generation_settings.n_ctx.
    assert_eq!(
        context_from_text(r#"{"default_generation_settings":{"n_ctx":4096}}"#),
        Some(4096)
    );
    // OpenRouter and vLLM name it per model.
    assert_eq!(
        context_from_text(r#"{"data":[{"id":"x","context_length":131072}]}"#),
        Some(131072)
    );
    // A one-field wrapper is still a clear statement.
    assert_eq!(context_from_text(r#"{"value":8192}"#), Some(8192));
    assert_eq!(context_from_text("not a number"), None);
    assert_eq!(context_from_text("0"), None);
}

#[test]
fn the_context_belongs_to_the_chosen_model() {
    let value: Value = serde_json::from_str(
        r#"{"data":[{"id":"small","context_length":4096},{"id":"big","context_length":32768}]}"#,
    )
    .expect("json");
    assert_eq!(context_for_model(&value, "big"), Some(32768));
    assert_eq!(context_for_model(&value, "small"), Some(4096));
    // Without a match the first entry is the best guess.
    assert_eq!(context_for_model(&value, ""), Some(4096));
}

#[test]
fn a_local_server_reports_its_context() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let address = listener.local_addr().expect("mock address");
    let worker = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
        let mut request_line = String::new();
        reader.read_line(&mut request_line).expect("request line");
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap_or(0) == 0 {
                break;
            }
            if line == "\r\n" || line == "\n" {
                break;
            }
        }
        assert!(
            request_line.contains("GET /props"),
            "request was {request_line:?}"
        );
        let body = r#"{"default_generation_settings":{"n_ctx":4096}}"#;
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
        api_key: String::new(),
        ..Default::default()
    };
    assert_eq!(context_from_server(&settings), Some(4096));
    worker.join().expect("mock server finished");
}

#[test]
fn koboldcpp_reports_its_context_at_the_root() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let address = listener.local_addr().expect("mock address");
    let worker = std::thread::spawn(move || {
        // /props and /api/v0/models are not there; the third request is
        // KoboldCpp's own endpoint, answering with a bare number.
        for answer in [None, None, Some("8192")] {
            let (mut stream, _) = listener.accept().expect("accept request");
            let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
            let mut request_line = String::new();
            reader.read_line(&mut request_line).expect("request line");
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 {
                    break;
                }
                if line == "\r\n" || line == "\n" {
                    break;
                }
            }
            let response = match answer {
                Some(body) => format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                ),
                None => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string(),
            };
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });
    let settings = AiSettings {
        base_url: format!("http://{address}/v1"),
        api_key: String::new(),
        ..Default::default()
    };
    assert_eq!(context_from_server(&settings), Some(8192));
    worker.join().expect("mock server finished");
}

#[test]
fn model_lists_come_in_two_shapes() {
    let openai = parse_models(r#"{"data":[{"id":"zeta"},{"id":"alpha"}]}"#).expect("openai shape");
    assert_eq!(openai, vec!["alpha", "zeta"]);
    let ollama = parse_models(r#"{"models":[{"name":"llama3.2"},{"name":"qwen2.5"}]}"#)
        .expect("ollama shape");
    assert_eq!(ollama, vec!["llama3.2", "qwen2.5"]);
    assert!(parse_models(r#"{"data":[]}"#).is_err());
    assert!(parse_models("<html>not json</html>").is_err());
}

#[test]
fn models_are_asked_from_the_endpoint() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let address = listener.local_addr().expect("mock address");
    let worker = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
        let mut request_line = String::new();
        reader.read_line(&mut request_line).expect("request line");
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap_or(0) == 0 {
                break;
            }
            if line == "\r\n" || line == "\n" {
                break;
            }
        }
        assert!(
            request_line.contains("GET /v1/models"),
            "request was {request_line:?}"
        );
        let body = r#"{"data":[{"id":"zeta-7b","context_length":8192},{"id":"alpha-3b","context_length":4096}]}"#;
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
        api_key: String::new(),
        model: "zeta-7b".to_string(),
        ..Default::default()
    };
    let models = list_models(&settings).expect("models");
    assert_eq!(models.names, vec!["alpha-3b", "zeta-7b"]);
    assert_eq!(models.context_limit, Some(8192));
    worker.join().expect("mock server finished");
}
