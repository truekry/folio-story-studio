//! How much of the context window the prompt, the history and the reply get.

use super::*;

#[test]
fn a_full_context_leaves_no_room_for_a_reply() {
    let mut settings = AiSettings {
        context_limit: 900,
        max_tokens: 700,
        ..Default::default()
    };
    let long = "word ".repeat(3000);
    assert!(reply_budget(&settings, &long, &[], "", settings.max_tokens).is_err());

    // A reply that would not fit is trimmed to what remains.
    settings.context_limit = 4096;
    let budget = reply_budget(&settings, &long, &[], "", settings.max_tokens).expect("budget");
    assert!(budget < 700, "budget was {budget}");
    assert!(budget >= 64);

    // Unknown window: nothing is touched.
    settings.context_limit = 0;
    assert_eq!(
        reply_budget(&settings, &long, &[], "", settings.max_tokens),
        Ok(700)
    );
}

#[test]
fn the_conversation_has_to_stay_under_the_fill_line() {
    let mut settings = AiSettings {
        context_limit: 10_000,
        ..Default::default()
    };
    // 85% of 10 000 leaves 8 500 tokens; the prompt and system text are
    // deducted before the conversation gets its share.
    let budget = history_budget(&settings, "short prompt");
    assert!(budget < 8_500 && budget > 7_000, "budget was {budget}");

    // The same numbers decide how much of the history travels.
    let history = vec![ChatMessage {
        role: "user".to_string(),
        text: "word ".repeat(2000),
        demo: false,
    }];
    assert!(reply_budget(&settings, "short prompt", &history, "", settings.max_tokens).is_ok());
    settings.context_limit = 2000;
    assert!(reply_budget(&settings, "short prompt", &history, "", settings.max_tokens).is_err());

    // Without a reported window the budget is unlimited.
    settings.context_limit = 0;
    assert_eq!(history_budget(&settings, "short prompt"), usize::MAX);
}

#[test]
fn a_zero_word_target_asks_for_the_whole_window() {
    let mut settings = AiSettings::default();
    // No reported window: the global reply cap is the safety net.
    assert_eq!(reply_token_wish(&settings, 0), settings.max_tokens);
    settings.context_limit = 8192;
    // A reported window turns "no limit" into the whole remaining room.
    assert_eq!(reply_token_wish(&settings, 0), u32::MAX);
    let room = reply_budget(&settings, "Tell me a story.", &[], "", u32::MAX).expect("room");
    assert!(room > settings.max_tokens, "room was {room}");
    assert!(room < 8192, "room was {room}");
    // A word target still respects the global cap.
    settings.max_tokens = 500;
    assert_eq!(reply_token_wish(&settings, 240), 500);
}

#[test]
fn a_length_target_decides_how_much_room_the_reply_gets() {
    // Roughly two tokens per word, plus room to finish the sentence.
    assert_eq!(target_reply_tokens(90), 280);
    assert_eq!(target_reply_tokens(240), 580);
    // A request without a target keeps the usual reply limit.
    let settings = AiSettings::default();
    assert_eq!(settings.max_tokens, 700);
}
