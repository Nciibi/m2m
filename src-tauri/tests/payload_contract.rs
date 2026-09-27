//! Cross-language payload contract.
//!
//! The frontend validates every Tauri event with a runtime guard in
//! `src/events.ts`. Those guards are a security boundary: if one rejects a
//! payload the backend legitimately produces, the corresponding message is
//! silently dropped and the app looks broken with no error anywhere.
//!
//! That is not hypothetical. `asChatMessage` required a 64-char hex
//! `sender_peer_key_hex`, but the field is documented as "Empty string for 1:1
//! messages (implicit from conversation)" and `ChatMessage::new` defaults it to
//! `String::new()`. Every direct message was therefore rejected — while the
//! whole test suite stayed green, because the fixtures were hand-written rather
//! than generated from the real serializer.
//!
//! These tests are the fix for that class of bug. Each one serializes the real
//! struct and asserts the exact JSON key set, so a rename, an added field, or a
//! change in nullability fails the Rust build. The mirrored TypeScript tests in
//! `src/__tests__/payloadContract.test.ts` feed the same values through the
//! frontend guards, so the two sides cannot drift apart unnoticed.

use serde_json::{json, Value};

use m2m_lib::commands::{ChatMessage, ConnectionEvent, GroupEvent, MessageEvent};

/// The complete set of keys serde emits for a `ChatMessage`.
///
/// Asserted as a sorted key list rather than a substring check, because a
/// missing key and a misspelled key are equally breaking.
fn chat_message_keys(v: &Value) -> Vec<String> {
    let mut keys: Vec<String> = v
        .as_object()
        .expect("ChatMessage must serialize to an object")
        .keys()
        .cloned()
        .collect();
    keys.sort();
    keys
}

fn expected_chat_message_keys() -> Vec<String> {
    let mut keys = vec![
        "id",
        "content",
        "direction",
        "timestamp",
        "read_at",
        "edited_at",
        "deleted",
        "expires_at",
        "reactions",
        "sender_peer_key_hex",
    ]
    .into_iter()
    .map(String::from)
    .collect::<Vec<_>>();
    keys.sort();
    keys
}

#[test]
fn chat_message_key_set_is_exactly_as_the_frontend_expects() {
    let msg = ChatMessage::new(
        "m1".to_string(),
        "hello".to_string(),
        "received".to_string(),
        1_700_000_000,
    );
    let v = serde_json::to_value(&msg).expect("ChatMessage serializes");
    assert_eq!(chat_message_keys(&v), expected_chat_message_keys());
}

#[test]
fn chat_message_always_emits_sender_peer_key_hex_even_when_empty() {
    // The field is NOT Option<String>. A 1:1 message carries "", and the
    // frontend guard must accept "" and an absent value, not just a 64-char
    // key. This test fails if the field is ever made optional or skipped.
    let mut msg = ChatMessage::new(
        "m1".to_string(),
        "direct".to_string(),
        "received".to_string(),
        1_700_000_000,
    );
    assert_eq!(msg.sender_peer_key_hex, "");

    let v = serde_json::to_value(&msg).expect("ChatMessage serializes");
    assert_eq!(
        v.get("sender_peer_key_hex"),
        Some(&json!("")),
        "a 1:1 message must serialize sender_peer_key_hex as an empty STRING, \
         not null and not absent"
    );

    // And a group message carries a real key in the same field.
    msg = msg.with_sender("c".repeat(64));
    let v = serde_json::to_value(&msg).expect("ChatMessage serializes");
    assert_eq!(v.get("sender_peer_key_hex"), Some(&json!("c".repeat(64))));
}

#[test]
fn chat_message_is_never_serialized_with_a_null_sender_key() {
    // `Option<String>` would serialize to null when None. The guard rejects
    // null, so this pins the two sides: the type must stay a plain String.
    let msg = ChatMessage::new(
        "m2".to_string(),
        "direct".to_string(),
        "sent".to_string(),
        1_700_000_001,
    );
    let v = serde_json::to_value(&msg).expect("ChatMessage serializes");
    assert!(
        !v.as_object().unwrap().contains_key("sender_peer_key_hex")
            || v["sender_peer_key_hex"].is_string(),
        "sender_peer_key_hex must never be null"
    );
}

#[test]
fn message_event_key_set_is_exactly_as_the_frontend_expects() {
    let ev = MessageEvent {
        peer_key_hex: "b".repeat(64),
        message: ChatMessage::new(
            "m1".to_string(),
            "direct".to_string(),
            "received".to_string(),
            1_700_000_000,
        ),
    };
    let v = serde_json::to_value(&ev).expect("MessageEvent serializes");
    let mut keys: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(keys, vec!["message", "peer_key_hex"]);

    // The nested message must keep its own key set.
    assert_eq!(chat_message_keys(&v["message"]), expected_chat_message_keys());
}

#[test]
fn group_event_peer_key_is_nullable_and_serializes_as_null_when_absent() {
    // Unlike ChatMessage, GroupEvent.peer_key_hex IS Option<String>. The
    // frontend guard already handles null here; this pins the asymmetry so a
    // well-meaning edit that makes it non-nullable (or vice versa) is caught.
    let ev = GroupEvent {
        group_id: "g1".to_string(),
        event_type: "member_left".to_string(),
        peer_key_hex: None,
    };
    let v = serde_json::to_value(&ev).expect("GroupEvent serializes");
    assert_eq!(v["peer_key_hex"], Value::Null);

    let ev = GroupEvent {
        peer_key_hex: Some("a".repeat(64)),
        ..ev
    };
    let v = serde_json::to_value(&ev).expect("GroupEvent serializes");
    assert_eq!(v["peer_key_hex"], json!("a".repeat(64)));
}

#[test]
fn connection_event_optional_fields_serialize_as_null() {
    let ev = ConnectionEvent {
        peer_key_hex: "a".repeat(64),
        state: "handshaking".to_string(),
        peer_fingerprint: None,
        peer_verified: false,
    };
    let v = serde_json::to_value(&ev).expect("ConnectionEvent serializes");
    let mut keys: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(
        keys,
        vec!["peer_fingerprint", "peer_key_hex", "peer_verified", "state"]
    );
    assert_eq!(v["peer_fingerprint"], Value::Null);
}
