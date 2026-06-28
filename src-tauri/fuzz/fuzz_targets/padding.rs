//! Fuzz target for message padding/unpadding.
//!
//! Feeds random byte sequences into unpad_message_variable.
//! Oracle invariants:
//! 1. Must NEVER panic
//! 2. If unpadding succeeds, the result must not exceed the input length
//! 3. Re-padding the unpadded result must produce output of the same length

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    match m2m_lib::crypto::unpad_message_variable(data) {
        Ok(ref plaintext) => {
            let pt_len = plaintext.len();
            let in_len = data.len();
            // Invariant: unpadded result must not exceed input length
            assert!(
                pt_len <= in_len,
                "unpadded length {} exceeds input length {}",
                pt_len,
                in_len
            );

            // Invariant: re-padding must succeed and produce consistent length
            let repadded = m2m_lib::crypto::pad_message_variable(plaintext);
            assert!(
                repadded.len() >= pt_len,
                "repadded length {} shorter than plaintext {}",
                repadded.len(),
                pt_len
            );

            // Verify the repadded data can be unpadded again to the same plaintext
            let re_unpadded = m2m_lib::crypto::unpad_message_variable(&repadded).unwrap();
            assert_eq!(re_unpadded, *plaintext,
                "round-trip unpad->pad->unpad produced different plaintext");
        }
        Err(_) => {}
    }
});
