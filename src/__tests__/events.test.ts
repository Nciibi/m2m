import { describe, it, expect } from "vitest";
import {
  asAppError,
  asCaptureWarning,
  asChatMessage,
  asConnectionEvent,
  asDeleteEvent,
  asEditEvent,
  asFileRequestEvent,
  asGroupEvent,
  asMessageEvent,
  asReactionEvent,
  asReconnectAttempt,
  asSecurityError,
  asStorageEvicted,
  asTransferErrorEvent,
  asTransferProgressEvent,
  asTypingEvent,
  RECONNECT_STATES,
} from "../events";

/**
 * Event payload validators.
 *
 * These are the boundary between peer-controlled data and React state. The
 * payloads contain message bodies (which reach `renderMarkdown`), filenames
 * (rendered and used as a save-path default), group names, reaction emojis
 * (used as object keys AND visible labels), and `state` strings that are
 * interpolated into CSS class names and drive view navigation.
 *
 * Every `listen()` call was previously `listen<any>`, which — because
 * TypeScript types are erased — provided no checking whatsoever.
 */

const KEY = "a".repeat(64);
const KEY2 = "b".repeat(64);

function message(over: Record<string, unknown> = {}) {
  return {
    id: "m1",
    content: "hello",
    direction: "received",
    timestamp: 1_700_000_000,
    read_at: null,
    edited_at: null,
    deleted: false,
    expires_at: null,
    reactions: {},
    sender_peer_key_hex: KEY,
    ...over,
  };
}

describe("asChatMessage", () => {
  it("accepts a well-formed message", () => {
    expect(asChatMessage(message())).not.toBeNull();
  });

  it("rejects a direction the app never produces", () => {
    // The bug this guards: `direction` was typed `string`, and the test
    // fixtures used "incoming"/"outgoing" — so the rendering tests exercised
    // a branch that could never run in production.
    expect(asChatMessage(message({ direction: "incoming" }))).toBeNull();
    expect(asChatMessage(message({ direction: "outgoing" }))).toBeNull();
    expect(asChatMessage(message({ direction: "SENT" }))).toBeNull();
    expect(asChatMessage(message({ direction: "sent" }))).not.toBeNull();
    expect(asChatMessage(message({ direction: "received" }))).not.toBeNull();
  });

  it("rejects a non-hex sender key", () => {
    expect(asChatMessage(message({ sender_peer_key_hex: "nope" }))).toBeNull();
    expect(asChatMessage(message({ sender_peer_key_hex: "a".repeat(63) }))).toBeNull();
  });

  it("rejects an oversized body", () => {
    // Bounds the work `renderMarkdown` has to do, and stops a peer pushing a
    // huge string into the DOM.
    expect(asChatMessage(message({ content: "x".repeat(64 * 1024 + 1) }))).toBeNull();
  });

  it("rejects a malformed reactions map", () => {
    expect(asChatMessage(message({ reactions: [] }))).toBeNull();
    expect(asChatMessage(message({ reactions: null }))).toBeNull();
    const longKey: Record<string, string[]> = {};
    longKey["x".repeat(64)] = [KEY];
    expect(asChatMessage(message({ reactions: longKey }))).toBeNull();
    expect(asChatMessage(message({ reactions: { ok: ["not-a-key"] } }))).toBeNull();
    expect(asChatMessage(message({ reactions: { ok: "not-an-array" } }))).toBeNull();
  });

  it("caps the number of distinct reactions", () => {
    const many: Record<string, string[]> = {};
    for (let i = 0; i < 40; i++) many[`e${i}`] = [KEY];
    expect(asChatMessage(message({ reactions: many }))).toBeNull();
  });

  it("rejects non-objects and null", () => {
    for (const bad of [null, undefined, 42, "x", [], true]) {
      expect(asChatMessage(bad)).toBeNull();
    }
  });
});

describe("asMessageEvent", () => {
  it("accepts a valid event", () => {
    const e = asMessageEvent({ peer_key_hex: KEY, message: message() });
    expect(e?.peer_key_hex).toBe(KEY);
  });

  it("rejects a valid message with a bad peer key", () => {
    expect(asMessageEvent({ peer_key_hex: "short", message: message() })).toBeNull();
  });

  it("rejects a good key with a bad message", () => {
    expect(asMessageEvent({ peer_key_hex: KEY, message: { id: "x" } })).toBeNull();
  });
});

describe("asConnectionEvent", () => {
  it("accepts both known states", () => {
    for (const state of ["established", "disconnected"]) {
      expect(
        asConnectionEvent({
          peer_key_hex: KEY,
          state,
          peer_fingerprint: "AA:BB",
          peer_verified: true,
        }),
      ).not.toBeNull();
    }
  });

  it("rejects an unknown state rather than defaulting it", () => {
    // `state` drives navigation, so guessing would be worse than refusing.
    expect(
      asConnectionEvent({ peer_key_hex: KEY, state: "compromised", peer_verified: false }),
    ).toBeNull();
    expect(
      asConnectionEvent({ peer_key_hex: KEY, state: 1, peer_verified: false }),
    ).toBeNull();
  });

  it("defaults peer_verified to false when absent", () => {
    // Matches the Rust `#[serde(default)]`.
    const e = asConnectionEvent({ peer_key_hex: KEY, state: "established" });
    expect(e?.peer_verified).toBe(false);
  });
});

describe("asReconnectAttempt", () => {
  it("accepts every state the backend emits", () => {
    // Includes "handshake_failed", which the old untyped handler ignored —
    // leaving the UI stuck on "Reconnecting (N/5)…" forever.
    for (const state of RECONNECT_STATES) {
      expect(
        asReconnectAttempt({
          peer_key_hex: KEY,
          attempt: 1,
          max_attempts: 5,
          delay_secs: 2,
          state,
        }),
      ).not.toBeNull();
    }
    expect(RECONNECT_STATES).toContain("handshake_failed");
  });

  it("rejects an unknown state", () => {
    expect(
      asReconnectAttempt({
        peer_key_hex: KEY,
        attempt: 1,
        max_attempts: 5,
        delay_secs: 2,
        state: "gave_up",
      }),
    ).toBeNull();
  });
});

describe("asFileRequestEvent", () => {
  it("accepts a valid request", () => {
    expect(
      asFileRequestEvent({
        peer_key_hex: KEY,
        transfer_id: "t1",
        filename: "report.pdf",
        total_size: 1024,
      }),
    ).not.toBeNull();
  });

  it("rejects a control character in the filename", () => {
    expect(
      asFileRequestEvent({
        peer_key_hex: KEY,
        transfer_id: "t1",
        filename: "bad\u0000name",
        total_size: 1,
      }),
    ).toBeNull();
  });

  it("rejects an oversized filename", () => {
    expect(
      asFileRequestEvent({
        peer_key_hex: KEY,
        transfer_id: "t1",
        filename: "x".repeat(513),
        total_size: 1,
      }),
    ).toBeNull();
  });
});

describe("asTransferProgressEvent", () => {
  const progress = {
    transfer_id: "t1",
    peer_key_hex: KEY,
    filename: "a.bin",
    total_size: 100,
    bytes_transferred: 50,
    chunks_completed: 1,
    chunks_total: 2,
    state: "receiving",
    speed_bytes_per_sec: 1024,
    estimated_remaining_secs: 1,
  };

  it("accepts a valid progress event", () => {
    expect(asTransferProgressEvent(progress)).not.toBeNull();
  });

  it("rejects a state outside the known set", () => {
    // `state` is interpolated into a className, so it must be constrained.
    expect(asTransferProgressEvent({ ...progress, state: '"><script>' })).toBeNull();
  });

  it("rejects negative counters", () => {
    expect(asTransferProgressEvent({ ...progress, bytes_transferred: -1 })).toBeNull();
    expect(asTransferProgressEvent({ ...progress, chunks_completed: -1 })).toBeNull();
  });
});

describe("asReactionEvent", () => {
  it("accepts a short emoji", () => {
    expect(
      asReactionEvent({ message_id: "m1", reaction: "👍", peer_key_hex: KEY, remove: false }),
    ).not.toBeNull();
  });

  it("rejects an unbounded reaction string", () => {
    // It becomes an object key and a visible label.
    expect(
      asReactionEvent({ message_id: "m1", reaction: "x".repeat(200), peer_key_hex: KEY, remove: false }),
    ).toBeNull();
  });

  it("requires a boolean remove flag", () => {
    expect(
      asReactionEvent({ message_id: "m1", reaction: "👍", peer_key_hex: KEY, remove: "yes" }),
    ).toBeNull();
  });
});

describe("asEditEvent", () => {
  it("accepts a valid edit", () => {
    expect(
      asEditEvent({ message_id: "m1", new_content: "edited", edited_at: 1, peer_key_hex: KEY }),
    ).not.toBeNull();
  });

  it("rejects an oversized replacement body", () => {
    expect(
      asEditEvent({
        message_id: "m1",
        new_content: "x".repeat(64 * 1024 + 1),
        edited_at: 1,
        peer_key_hex: KEY,
      }),
    ).toBeNull();
  });
});

describe("asTypingEvent", () => {
  it("accepts true and false", () => {
    expect(asTypingEvent({ peer_key_hex: KEY, typing: true })).not.toBeNull();
    expect(asTypingEvent({ peer_key_hex: KEY, typing: false })).not.toBeNull();
  });

  it("rejects a non-boolean", () => {
    expect(asTypingEvent({ peer_key_hex: KEY, typing: "true" })).toBeNull();
  });
});

describe("asGroupEvent", () => {
  it("accepts known event types", () => {
    for (const event_type of ["created", "invited", "member_added", "member_removed", "member_left", "name_changed"]) {
      expect(asGroupEvent({ group_id: "g1", event_type, peer_key_hex: null })).not.toBeNull();
    }
  });

  it("rejects an unknown event type", () => {
    expect(asGroupEvent({ group_id: "g1", event_type: "exfiltrate" })).toBeNull();
  });

  it("rejects a malformed peer key", () => {
    expect(asGroupEvent({ group_id: "g1", event_type: "created", peer_key_hex: "short" })).toBeNull();
  });
});

describe("asDeleteEvent", () => {
  it("accepts a valid delete", () => {
    expect(asDeleteEvent({ message_id: "m1", peer_key_hex: KEY })).not.toBeNull();
  });

  it("rejects a malformed peer key", () => {
    expect(asDeleteEvent({ message_id: "m1", peer_key_hex: "nope" })).toBeNull();
  });
});

describe("asTransferErrorEvent", () => {
  it("accepts and bounds the error text", () => {
    expect(asTransferErrorEvent({ transfer_id: "t1", error: "disk full" })).not.toBeNull();
    expect(
      asTransferErrorEvent({ transfer_id: "t1", error: "x".repeat(513) }),
    ).toBeNull();
  });

  it("keeps `error` a string so the failure toast is never dropped", () => {
    // The regression this guards: the backend has an `AppError` and it would
    // be natural to emit it wholesale. That makes `isDisplayText` fail, the
    // guard returns null, and the user sees a transfer vanish with no
    // explanation at all — strictly worse than an ugly message.
    const p = { transfer_id: "t1", error: { code: "io", message: "disk full" } };
    expect(asTransferErrorEvent(p)).toBeNull();
  });

  it("carries the optional error_code through", () => {
    const parsed = asTransferErrorEvent({
      transfer_id: "t1",
      error: "connection reset",
      error_code: "network.io",
    });
    expect(parsed?.error_code).toBe("network.io");
    // Absent code is fine — it was added with the error taxonomy.
    expect(asTransferErrorEvent({ transfer_id: "t1", error: "x" })?.error_code).toBeUndefined();
  });

  it("rejects a malformed error_code rather than passing it through", () => {
    expect(
      asTransferErrorEvent({ transfer_id: "t1", error: "x", error_code: 42 }),
    ).toBeNull();
    expect(
      asTransferErrorEvent({ transfer_id: "t1", error: "x", error_code: "" }),
    ).toBeNull();
  });
});

describe("asCaptureWarning", () => {
  it("accepts a bounded list", () => {
    expect(asCaptureWarning({ active: ["obs64.exe"] })).not.toBeNull();
    expect(asCaptureWarning({ active: [] })).not.toBeNull();
  });

  it("rejects an unbounded list", () => {
    expect(asCaptureWarning({ active: Array(64).fill("x") })).toBeNull();
  });
});

describe("asSecurityError", () => {
  it("accepts a bounded message", () => {
    expect(
      asSecurityError({ source: "screen_capture_protection", message: "failed to apply" }),
    ).not.toBeNull();
  });

  it("rejects an oversized source", () => {
    expect(asSecurityError({ source: "x".repeat(65), message: "m" })).toBeNull();
  });
});

/**
 * `m2m://storage-evicted` announces that the storage cap permanently destroyed
 * history. It is the only notice the user gets that messages are gone, so the
 * guard's job is to make sure a malformed payload is *dropped* rather than
 * rendered — and, more subtly, that a real one is not dropped.
 */
describe("asStorageEvicted", () => {
  const valid = {
    messages_evicted: 42,
    group_messages_evicted: 7,
    bytes_freed: 5_368_709_120,
    overrode_retention: ["conv-1", "conv-2"],
  };

  it("accepts the real payload shape", () => {
    expect(asStorageEvicted(valid)).toEqual(valid);
  });

  it("accepts an eviction that overrode nothing", () => {
    // `overrode_retention: []` is the overwhelmingly common case. A guard that
    // only passed with a non-empty list would mean the user is almost never
    // told about the eviction.
    const p = asStorageEvicted({ ...valid, overrode_retention: [] });
    expect(p).not.toBeNull();
    expect(p?.overrode_retention).toEqual([]);
  });

  it("rejects a non-array overrode_retention rather than coercing it", () => {
    // `asArray` turns a non-array into `[]`, so using it here would turn a
    // malformed payload into a *valid* one that silently drops the
    // conversation list — the exact information the user needs in order to
    // know which retention policy was overridden.
    for (const bad of ["conv-1", 7, null, { "0": "conv-1", length: 1 }, undefined]) {
      expect(asStorageEvicted({ ...valid, overrode_retention: bad })).toBeNull();
    }
  });

  it("rejects a conversation id with control characters", () => {
    expect(
      asStorageEvicted({ ...valid, overrode_retention: ["ok", "bad name"] }),
    ).toBeNull();
    expect(asStorageEvicted({ ...valid, overrode_retention: ["bad\nname"] })).toBeNull();
  });

  it("rejects a non-numeric or negative count", () => {
    // u32/u64 checks, not mere `typeof === "number"`. A negative byte count or
    // a fractional message count would be rendered straight into the notice
    // text, where it becomes "freed -1 bytes".
    expect(asStorageEvicted({ ...valid, messages_evicted: -1 })).toBeNull();
    expect(asStorageEvicted({ ...valid, messages_evicted: 1.5 })).toBeNull();
    expect(asStorageEvicted({ ...valid, messages_evicted: "42" })).toBeNull();
    expect(asStorageEvicted({ ...valid, group_messages_evicted: -3 })).toBeNull();
    expect(asStorageEvicted({ ...valid, bytes_freed: -1 })).toBeNull();
    expect(asStorageEvicted({ ...valid, messages_evicted: 2 ** 32 })).toBeNull();
  });

  it("accepts bytes_freed above u32", () => {
    // A 10 GiB cap can free more than 4 GiB in one pass. Narrowing this to u32
    // would silently drop exactly the notice a large eviction produces.
    const p = asStorageEvicted({ ...valid, bytes_freed: 2 ** 33 });
    expect(p).not.toBeNull();
    expect(p?.bytes_freed).toBe(2 ** 33);
  });

  it("rejects a missing field rather than defaulting it", () => {
    // Every field is emitted by the Rust side, so an absent one is a version
    // skew — and the message would then claim a certainty the payload does not
    // carry.
    for (const key of Object.keys(valid) as (keyof typeof valid)[]) {
      const partial: Record<string, unknown> = { ...valid };
      delete partial[key];
      expect(asStorageEvicted(partial)).toBeNull();
    }
  });

  it("rejects non-objects", () => {
    for (const bad of [null, undefined, 42, "m2m://storage-evicted", []]) {
      expect(asStorageEvicted(bad)).toBeNull();
    }
  });
});

describe("cross-field invariants", () => {
  it("uses a distinct key for the second peer", () => {
    // Guards the fixtures themselves: if KEY and KEY2 were accidentally the
    // same, a test asserting "the other peer" would pass vacuously.
    expect(KEY).not.toBe(KEY2);
    expect(KEY).toHaveLength(64);
  });
});

describe("asChatMessage — 1:1 messages", () => {
  // The Rust `ChatMessage.sender_peer_key_hex` field is documented as "Empty
  // string for 1:1 messages (implicit from conversation)" and `ChatMessage::new`
  // defaults it to `String::new()`. A validator that requires a 64-char key
  // therefore rejects EVERY direct message, and the chat listener silently drops
  // all 1:1 traffic.
  //
  // Every earlier fixture in this file supplied a group-style key, so the suite
  // could not catch it. These cases pin the actual 1:1 shape.
  const direct = (over: Record<string, unknown> = {}) => ({
    id: "m1",
    content: "hello",
    direction: "received",
    timestamp: 1_700_000_000,
    read_at: null,
    edited_at: null,
    deleted: false,
    expires_at: null,
    reactions: {},
    sender_peer_key_hex: "",
    ...over,
  });

  it("accepts a direct message with an empty sender key", () => {
    const m = asChatMessage(direct());
    expect(m).not.toBeNull();
    expect(m?.sender_peer_key_hex).toBe("");
  });

  it("accepts a direct message that omits the field entirely", () => {
    const { sender_peer_key_hex: _omitted, ...withoutKey } = direct();
    expect(asChatMessage(withoutKey)).not.toBeNull();
  });

  it("still accepts a group message with a real sender key", () => {
    const key = "a".repeat(64);
    expect(asChatMessage(direct({ sender_peer_key_hex: key }))?.sender_peer_key_hex).toBe(key);
  });

  // The empty string is a legitimate sentinel, not a licence to accept junk.
  it("rejects a malformed non-empty sender key", () => {
    for (const bad of ["not-a-key", "z".repeat(64), "A".repeat(63), " ".repeat(64), 42, null]) {
      expect(asChatMessage(direct({ sender_peer_key_hex: bad }))).toBeNull();
    }
  });

  it("rejects a sender key with trailing whitespace", () => {
    expect(asChatMessage(direct({ sender_peer_key_hex: `${"a".repeat(64)} ` }))).toBeNull();
  });
});

/**
 * `asAppError` — the command-failure boundary.
 *
 * Every `#[tauri::command]` now returns `Result<T, AppError>`, so every
 * `invoke()` rejection is `{ code, message }` rather than a bare string. The
 * `code` is what callers switch on, which makes it as sensitive as a payload
 * field that drives navigation: if arbitrary text can land there, a `switch` on
 * it is not a real decision.
 */
describe("asAppError", () => {
  it("accepts the shape the Rust side serialises", () => {
    const e = asAppError({ code: "family.unreachable", message: "address is stale" });
    expect(e).toEqual({ code: "family.unreachable", message: "address is stale" });
  });

  it("accepts an unknown code without rejecting it", () => {
    // The backend adds codes freely; a guard that rejected unknown ones would
    // turn every newly-added code into a silent "Unknown error" for the user.
    expect(asAppError({ code: "brand.new_code", message: "hi" })?.code).toBe("brand.new_code");
  });

  it("rejects non-objects and strings", () => {
    // A bare string is what commands rejected with *before* the taxonomy
    // landed. It must not be silently upgraded into a code.
    for (const bad of [null, undefined, 42, "boom", true, []]) {
      expect(asAppError(bad)).toBeNull();
    }
  });

  it("rejects a missing or non-string code", () => {
    expect(asAppError({ message: "no code" })).toBeNull();
    expect(asAppError({ code: 7, message: "x" })).toBeNull();
    expect(asAppError({ code: null, message: "x" })).toBeNull();
    expect(asAppError({ code: "", message: "x" })).toBeNull();
  });

  it("rejects a code carrying control characters or unbounded length", () => {
    // A code containing a newline is a smuggling attempt against anything that
    // later logs or renders it.
    expect(asAppError({ code: `bad${String.fromCharCode(10)}code`, message: "x" })).toBeNull();
    expect(asAppError({ code: "c".repeat(129), message: "x" })).toBeNull();
  });

  it("rejects a missing, over-long or control-character message", () => {
    expect(asAppError({ code: "invalid_input" })).toBeNull();
    expect(asAppError({ code: "invalid_input", message: "x".repeat(513) })).toBeNull();
    expect(
      asAppError({ code: "invalid_input", message: `a${String.fromCharCode(7)}b` }),
    ).toBeNull();
    expect(asAppError({ code: "invalid_input", message: 42 })).toBeNull();
  });
});
