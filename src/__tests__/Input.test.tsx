import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import Input from "../components/ui/Input";

/**
 * `Input` composes rather than replaces `onFocus`/`onBlur`.
 *
 * The internal handlers toggle `input-wrap--focused` / `input-wrap--error` on
 * the wrapper, and the caller's handler runs alongside. Both halves matter:
 *
 *  - `{...rest}` used to be spread *after* `onFocus`/`onBlur`, so any caller
 *    passing its own silently replaced the internal ones and the focus ring
 *    stopped working. `VaultView` passes `onFocus` on both passphrase fields
 *    (to drive the on-screen keyboard), so the two fields that most need a
 *    visible focus ring never had one.
 *  - Moving the spread earlier is not the fix either: that stops the caller's
 *    handler firing at all, breaking the on-screen keyboard.
 *
 * There was no `Input` test file at all, which is why either version of this
 * bug could ship.
 */
describe("Input focus handling", () => {
  function wrapper(input: HTMLElement): HTMLElement {
    const wrap = input.closest(".input-wrap");
    if (!(wrap instanceof HTMLElement)) throw new Error("no .input-wrap ancestor");
    return wrap;
  }

  it("toggles the focused class on focus and blur", async () => {
    const user = userEvent.setup();
    render(<Input value="" onChange={() => {}} placeholder="p" />);
    const input = screen.getByPlaceholderText("p");

    expect(wrapper(input)).not.toHaveClass("input-wrap--focused");

    await user.click(input);
    expect(wrapper(input)).toHaveClass("input-wrap--focused");

    await user.tab();
    expect(wrapper(input)).not.toHaveClass("input-wrap--focused");
  });

  it("still calls a caller-supplied onFocus", async () => {
    const user = userEvent.setup();
    const onFocus = vi.fn();
    render(<Input value="" onChange={() => {}} onFocus={onFocus} placeholder="p" />);

    await user.click(screen.getByPlaceholderText("p"));

    expect(onFocus).toHaveBeenCalledTimes(1);
  });

  it("composes the internal ring WITH a caller onFocus", async () => {
    // The regression this file exists for: the caller's handler used to win and
    // the focus ring silently vanished.
    const user = userEvent.setup();
    const onFocus = vi.fn();
    render(<Input value="" onChange={() => {}} onFocus={onFocus} placeholder="p" />);
    const input = screen.getByPlaceholderText("p");

    await user.click(input);

    expect(onFocus).toHaveBeenCalledTimes(1);
    expect(wrapper(input)).toHaveClass("input-wrap--focused");
  });

  it("composes the internal ring WITH a caller onBlur", async () => {
    const user = userEvent.setup();
    const onBlur = vi.fn();
    render(<Input value="" onChange={() => {}} onBlur={onBlur} placeholder="p" />);
    const input = screen.getByPlaceholderText("p");

    await user.click(input);
    expect(wrapper(input)).toHaveClass("input-wrap--focused");

    await user.tab();
    expect(onBlur).toHaveBeenCalledTimes(1);
    expect(wrapper(input)).not.toHaveClass("input-wrap--focused");
  });

  it("drops the error class on focus and restores it on blur", async () => {
    const user = userEvent.setup();
    render(<Input value="" onChange={() => {}} error="bad passphrase" placeholder="p" />);
    const input = screen.getByPlaceholderText("p");

    expect(wrapper(input)).toHaveClass("input-wrap--error");

    await user.click(input);
    expect(wrapper(input)).not.toHaveClass("input-wrap--error");

    await user.tab();
    expect(wrapper(input)).toHaveClass("input-wrap--error");
  });

  it("renders the error message", () => {
    render(<Input value="" onChange={() => {}} error="Passphrases do not match." placeholder="p" />);
    expect(screen.getByText("Passphrases do not match.")).toBeInTheDocument();
  });
});