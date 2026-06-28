//! Fuzz target for message padding/unpadding.
//!
//! Feeds random byte sequences into unpad_message_variable.
//! Oracle invariants:
//! 1. Must NEVER panic (sodiumoxide init is handled)
//! 2. If unpadding succeeds, the result must not exceed the input length
//! 3. Re-padding the unpadded result must produce output of the same length

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // unpad_message_variable should never panic, even on garbage data
    match m2m::crypto::unpad_message_variable(data) {
        Ok(plaintext) => {
            // Invariant: unpadded result must not exceed input length
            assert!(plaintext.len() <= data.len(),
                "unpadded length {} exceeds input length {}",
                plaintext.len(), data.len());

            // Invariant: re-padding must succeed and produce consistent length
            let repadded = m2m::crypto::pad_message_variable(&plaintext);
            assert!(repadded.len() >= plaintext.len(),
                "repadded length {} shorter than plaintext {}",
                repadded.len(), plaintext.len());

            // Verify the repadded data can be unpadded again to the same plaintext
            let re_unpadded = m2m::crypto::unpad_message_variable(&repadded).unwrap();
            assert_eq!(re_unpadded, plaintext,
                "round-trip unpad→pad→unpad produced different plaintext");
        }
        Err(_) => {
            // Expected for invalid padding — no action needed
        }
    }
});
