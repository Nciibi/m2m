import { describe, it, expect, vi, beforeEach } from "vitest";
import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";

const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: any[]) => mockInvoke(...a) }));

import { render } from "./setup";

/** The dialog's inputs are addressed by id: `getByLabelText` also matches the
 *  dialog's own `aria-label`, which contains the same phrase. */
function input(id: string): HTMLInputElement {
  const el = document.getElementById(id);
  if (!el) throw new Error(`no element with id ${id}`);
  return el as HTMLInputElement;
}
import { ConfirmDialog, DuressPassphraseDialog } from "../components/ui/ConfirmDialog";

/**
 * Destructive-action dialogs.
 *
 * These replaced `window.prompt` + `window.confirm` for the two irreversible,
 * life-safety actions in the product:
 *
 *   - arming the panic wipe (Ctrl+Alt+Shift+W destroys everything on press)
 *   - registering a duress passphrase (entering it at unlock silently wipes)
 *
 * Native dialogs were wrong for this specific product, not just old-fashioned:
 * they cannot be styled, cannot be localised, and are a recognised target for
 * UI spoofing. Someone under coercion has to be able to *read* what they are
 * agreeing to in a language they actually read.
 */

describe("ConfirmDialog", () => {
  function Host({ onConfirm, onCancel }: { onConfirm: () => void; onCancel: () => void }) {
    const [open, setOpen] = useState(false);
    return (
      <>
        <button onClick={() => setOpen(true)}>Open</button>
        <ConfirmDialog
          open={open}
          title="Arm panic hotkey?"
          body="This deletes all local data."
          confirmLabel="Arm it"
          cancelLabel="Cancel"
          destructive
          onConfirm={() => {
            onConfirm();
            setOpen(false);
          }}
          onCancel={() => {
            onCancel();
            setOpen(false);
          }}
        />
      </>
    );
  }

  beforeEach(() => vi.clearAllMocks());

  it("states the consequence before offering the action", async () => {
    const user = userEvent.setup();
    const onConfirm = vi.fn();
    render(<Host onConfirm={onConfirm} onCancel={vi.fn()} />);
    await user.click(screen.getByText("Open"));

    // The body must be visible *and* the dialog must not have auto-confirmed.
    expect(screen.getByText("This deletes all local data.")).toBeInTheDocument();
    expect(onConfirm).not.toHaveBeenCalled();
  });

  it("does not run the action when cancelled", async () => {
    const user = userEvent.setup();
    const onConfirm = vi.fn();
    const onCancel = vi.fn();
    render(<Host onConfirm={onConfirm} onCancel={onCancel} />);
    await user.click(screen.getByText("Open"));
    await user.click(screen.getByText("Cancel"));
    expect(onCancel).toHaveBeenCalledTimes(1);
    expect(onConfirm).not.toHaveBeenCalled();
  });

  it("runs the action exactly once on confirm", async () => {
    const user = userEvent.setup();
    const onConfirm = vi.fn();
    render(<Host onConfirm={onConfirm} onCancel={vi.fn()} />);
    await user.click(screen.getByText("Open"));
    await user.click(screen.getByText("Arm it"));
    expect(onConfirm).toHaveBeenCalledTimes(1);
  });

  it("closes on Escape without confirming", async () => {
    const user = userEvent.setup();
    const onConfirm = vi.fn();
    const onCancel = vi.fn();
    render(<Host onConfirm={onConfirm} onCancel={onCancel} />);
    await user.click(screen.getByText("Open"));
    await user.keyboard("{Escape}");
    expect(onCancel).toHaveBeenCalled();
    expect(onConfirm).not.toHaveBeenCalled();
  });
});

describe("DuressPassphraseDialog", () => {
  function Host({ onSubmit }: { onSubmit: (v: string) => Promise<void> }) {
    const [open, setOpen] = useState(false);
    return (
      <>
        <button onClick={() => setOpen(true)}>Set…</button>
        <DuressPassphraseDialog
          open={open}
          onClose={() => setOpen(false)}
          onSubmit={async (v) => {
            await onSubmit(v);
            setOpen(false);
          }}
        />
      </>
    );
  }

  beforeEach(() => vi.clearAllMocks());

  it("states the irreversibility and the absence of an unlock-time warning", async () => {
    const user = userEvent.setup();
    render(<Host onSubmit={vi.fn()} />);
    await user.click(screen.getByText("Set…"));

    const body = screen.getByText(
      /silently deletes all local data/i,
    );
    expect(body).toBeInTheDocument();
    // The "no confirmation at unlock" property is the whole point of the
    // feature, so it has to be stated in the dialog rather than implied.
    expect(screen.getByText(/cannot be undone/i)).toBeInTheDocument();
  });

  it("does not submit a passphrase shorter than 12 characters", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();
    render(<Host onSubmit={onSubmit} />);
    await user.click(screen.getByText("Set…"));

    await user.type(input("duress-passphrase"), "short");
    await user.type(input("duress-passphrase-confirm"), "short");

    // The confirm button must be disabled: the native prompt only enforced
    // this in Rust, *after* the user had already confirmed an irreversible act.
    expect(screen.getByText("Wipe on this passphrase").closest("button")).toBeDisabled();
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("does not submit when the confirmation does not match", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();
    render(<Host onSubmit={onSubmit} />);
    await user.click(screen.getByText("Set…"));

    await user.type(input("duress-passphrase"), "correct-horse-battery");
    await user.type(input("duress-passphrase-confirm"), "correct-horse-batteru");

    expect(screen.getByRole("alert")).toHaveTextContent(/do not match/i);
    expect(screen.getByText("Wipe on this passphrase").closest("button")).toBeDisabled();
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("submits a valid, matching passphrase", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<Host onSubmit={onSubmit} />);
    await user.click(screen.getByText("Set…"));

    const value = "correct-horse-battery-staple";
    await user.type(input("duress-passphrase"), value);
    await user.type(input("duress-passphrase-confirm"), value);
    await user.click(screen.getByText("Wipe on this passphrase"));

    await waitFor(() => expect(onSubmit).toHaveBeenCalledWith(value));
  });

  it("keeps the typed value and shows the error when the backend rejects it", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockRejectedValue("passphrase too similar to your main one");
    render(<Host onSubmit={onSubmit} />);
    await user.click(screen.getByText("Set…"));

    const value = "correct-horse-battery-staple";
    const passphraseInput = input("duress-passphrase");
    await user.type(input, value);
    await user.type(input("duress-passphrase-confirm"), value);
    await user.click(screen.getByText("Wipe on this passphrase"));

    // The dialog must NOT close on failure, and must not silently clear what
    // the user typed — losing a passphrase they just carefully invented is
    // worse than showing them the error.
    await waitFor(() =>
      expect(screen.getByText(/too similar/i)).toBeInTheDocument(),
    );
    expect(passphraseInput).toHaveValue(value);
  });

  it("does not use a native dialog", async () => {
    // Regression guard: this whole change exists because of these two calls.
    const promptSpy = vi.spyOn(window, "prompt");
    const confirmSpy = vi.spyOn(window, "confirm");
    const user = userEvent.setup();
    render(<Host onSubmit={vi.fn()} />);
    await user.click(screen.getByText("Set…"));
    expect(promptSpy).not.toHaveBeenCalled();
    expect(confirmSpy).not.toHaveBeenCalled();
    promptSpy.mockRestore();
    confirmSpy.mockRestore();
  });
});
