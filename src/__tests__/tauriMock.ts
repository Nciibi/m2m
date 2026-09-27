/**
 * Typed stand-ins for the Tauri IPC boundary, for use in tests.
 *
 * The production code validates every event payload with a runtime guard from
 * `src/events.ts`, which means a handler receives an *untrusted* value. Typing
 * the mock handler as `(e: any)` — which is what these tests did before —
 * quietly asserted the opposite: that the payload is already the right shape.
 * `unknown` is the honest type here. It forces each test to build a real
 * payload object, and it means a test that feeds a malformed payload is
 * genuinely exercising the validator rather than being handed a type error.
 */

/** A registered `listen` callback. Receives an unvalidated payload. */
export type MockEventHandler = (event: unknown) => void;

/** The `eventHandlers` registry shared between the `listen` mock and tests. */
export type EventHandlerRegistry = Map<string, MockEventHandler>;

/**
 * Wrap a `vi.fn()` as the Tauri `invoke` mock.
 *
 * Tauri commands are `Result<T, String>` on the Rust side, so a rejected call
 * is a bare string. Tests that simulate failure should `mockRejectedValue("…")`
 * with a string rather than an `Error` — see `rejectWith()` below.
 */
export function asInvokeMock(fn: (...args: never[]) => unknown) {
  return (...args: unknown[]): Promise<unknown> => {
    return Promise.resolve(fn(...(args as never[])));
  };
}

/** Build the string rejection that `Result<_, String>` produces. */
export function rejectWith(message: string): Promise<never> {
  return Promise.reject(message);
}

/**
 * Recursively-optional version of `T`.
 *
 * Context mocks in these tests intentionally populate only the fields the
 * component under test actually reads. Typing them as the full `T` would force
 * every fixture to spell out all ~10 fields of a `ChatMessage`, most of which
 * are irrelevant to the assertion — and the fields that *are* relevant would
 * drown in the noise. `DeepPartial` states the real intent: this is a partial
 * stand-in, not a full value.
 */
export type DeepPartial<T> = T extends (infer U)[]
  ? DeepPartial<U>[]
  : T extends object
    ? { [K in keyof T]?: DeepPartial<T[K]> }
    : T;
