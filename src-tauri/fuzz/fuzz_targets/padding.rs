//! Fuzz target: message padding (traffic-analysis layer).
//! `unpad_message_variable` parses an attacker-influenced length suffix —
//! must reject malformed input with Err, never panic or over-allocate.
//! Also verifies the roundtrip property on anything that unpads cleanly.

#![no_main]


libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    if let Ok(unpadded) = m2m_lib::crypto::unpad_message_variable(data) {
        // Roundtrip property: re-padding the recovered plaintext must
        // produce EXACTLY the original padded length (tier-deterministic),
        // and unpadding that must return the same plaintext.
        let repadded = m2m_lib::crypto::pad_message_variable(&unpadded);
        assert_eq!(
            repadded.len(),
            data.len(),
            "pad/unpad not length-deterministic: {} vs {}",
            data.len(),
            repadded.len()
        );
        let again = m2m_lib::crypto::unpad_message_variable(&repadded)
            .expect("repadded input must unpad");
        assert_eq!(again, unpadded, "roundtrip plaintext mismatch");
    }
});