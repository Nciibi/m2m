# M2M Fuzzing (security roadmap §4 — Assurance)

Targets the internet-facing parsers: frame parsing, MessagePack wire
structs, message padding, and DHT bootstrap responses.

## Layout

- `fuzz_targets/frame_parser.rs` — length-prefixed frame parser
  (`network::read_frame`), the first code remote bytes touch. Parsed
  bodies are additionally fed to their wire-struct deserializer.
- `fuzz_targets/wire_structs.rs` — every attacker-reachable MessagePack
  struct + packet-type/version byte dispatch totality.
- `fuzz_targets/padding.rs` — `unpad_message_variable` + roundtrip property.
- `fuzz_targets/dht_parser.rs` — DHT message/peer-entry parsers.

## Running

Requires **nightly Rust** and a libFuzzer-supported OS (**Linux** — use
WSL on Windows; plain MSVC `cargo build` of these targets fails at LINK
with "entry point must be defined" because libFuzzer's driver is only
wired in by cargo-fuzz's sanitizer flags):

```sh
cd src-tauri/fuzz
cargo +nightly fuzz run frame_parser -- -max_total_time=300
cargo +nightly fuzz run wire_structs -- -max_total_time=300
cargo +nightly fuzz run padding     -- -max_total_time=300
cargo +nightly fuzz run dht_parser  -- -max_total_time=300
```

`cargo check` (any toolchain, any OS) type-checks all targets without
linking — use it for quick validation after edits.

Crashes land in `crash-*` files — minimize with
`cargo +nightly fuzz tmin crash-*`, then convert to a regression test in
`src-tauri/src/protocol_fuzz_regression.rs`.

## CI without nightly

The same target logic runs deterministically in normal CI via the
regression tests (`protocol_fuzz_regression`, `dht_fuzz_regression`) which
feed fixed malformed inputs — no panics allowed. These run on Windows too,
so a parser regression cannot merge even where libFuzzer isn't available.

## Found so far

- `read_frame_impl` panicked on frames declaring `< 2` payload bytes
  (`payload[0]` index OOB) — remote DoS, FIXED (FrameTooSmall error).
