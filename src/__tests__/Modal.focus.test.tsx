import { describe, it, expect, vi, beforeEach } from "vitest";
import { screen } from "@testing-library/react";
import { render } from "./setup";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import Modal from "../components/ui/Modal";

/**
 * Modal focus management.
 *
 * The effect that installs the focus trap and the initial focus used to list
 * `onClose` in its dependency array. Every caller passes an inline arrow
 * (`onClose={() => setShowAdd(false)}`), so that identity changed on every
 * parent render — which meant:
 *
 *   1. typing in a text field re-ran the effect,
 *   2. which overwrote `previousFocus` with the element *inside* the dialog,
 *   3. which re-fired the `requestAnimationFrame` that focuses the FIRST
 *      field,
 *   4. so focus jumped back to the top field on every keystroke.
 *
 * Net effect: the user physically could not type a nickname into the
 * "Add to Family" dialog. This is not a cosmetic a11y nit — it makes a core
 * flow unusable, and it is exactly the kind of bug a presence-only test
 * (`expect(dialog).toBeInTheDocument()`) can never catch.
 */
describe("Modal focus management", () => {
  function Host({ withField = true }: { withField?: boolean }) {
    const [open, setOpen] = useState(false);
    const [name, setName] = useState("");
    return (
      <div>
        <button onClick={() => setOpen(true)}>Open</button>
        <span data-testid="name">{name}</span>
        {/* Inline arrow on purpose — this is what every real caller does and
            what used to trigger the bug. */}
        <Modal open={open} onClose={() => setOpen(false)} title="Add">
          {withField && (
            <input
              aria-label="nickname"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          )}
        </Modal>
      </div>
    );
  }

  beforeEach(() => {
    vi.useRealTimers();
  });

  it("keeps focus in the text field while typing", async () => {
    const user = userEvent.setup();
    render(<Host />);
    await user.click(screen.getByText("Open"));

    const input = screen.getByLabelText("nickname");
    await user.click(input);
    expect(document.activeElement).toBe(input);

    // Type several characters. Before the fix, focus was yanked back to the
    // first focusable element after the first character, so everything from
    // the second onward landed nowhere.
    await user.type(input, "alice");
    expect(screen.getByTestId("name")).toHaveTextContent("alice");
    expect(document.activeElement).toBe(input);
  });

  it("restores focus to the trigger when the dialog closes", async () => {
    const user = userEvent.setup();
    render(<Host />);
    const trigger = screen.getByText("Open");
    await user.click(trigger);

    const input = screen.getByLabelText("nickname");
    await user.click(input);
    // Escape closes via the ref'd onClose.
    await user.keyboard("{Escape}");

    expect(screen.queryByLabelText("nickname")).not.toBeInTheDocument();
    expect(document.activeElement).toBe(trigger);
  });

  it("closes on Escape", async () => {
    const user = userEvent.setup();
    render(<Host />);
    await user.click(screen.getByText("Open"));
    expect(screen.getByLabelText("nickname")).toBeInTheDocument();
    await user.keyboard("{Escape}");
    expect(screen.queryByLabelText("nickname")).not.toBeInTheDocument();
  });

  it("traps Tab within the dialog", async () => {
    const user = userEvent.setup();
    render(
      <Modal open onClose={() => {}} title="Add">
        <input aria-label="nickname" />
      </Modal>,
    );
    const input = screen.getByLabelText("nickname");
    await user.click(input);

    // Tabbing should cycle inside the dialog, never escaping to <body>.
    for (let i = 0; i < 6; i++) {
      await user.tab();
      const inside = dialogRef()?.contains(document.activeElement);
      expect(inside).toBe(true);
    }
  });
});

function dialogRef(): HTMLElement | null {
  return document.querySelector('[aria-modal="true"]');
}
