//! Fuzz target: DHT wire parsers (bootstrap-node-facing).
//! `parse_dht_message` and `parse_node_response` handle bytes from remote
//! bootstrap nodes — must reject malformed input with Err, never panic,
//! and produce sane address values.

#![no_main]


libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    use m2m_lib::dht;

    if let Ok((typ, body)) = dht::parse_dht_message(data) {
        // NODE_RESPONSE bodies get the peer-entry parser.
        let _ = dht::parse_node_response(body);

        // typ is validated by parse_dht_message; nothing to assert beyond
        // "did not panic", but touch it so dead-code lint stays honest.
        std::hint::black_box(typ);
    }
});