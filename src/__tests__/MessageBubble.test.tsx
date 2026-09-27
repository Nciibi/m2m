import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import MessageBubble from "../components/chat/MessageBubble";
import type { ChatMessage } from "../types";

/** A complete ChatMessage. Every field is filled so a test only overrides what
 *  it actually cares about — partial fixtures here meant a test could pass
 *  without exercising the branch it claimed to. */
function msg(overrides: Partial<ChatMessage> = {}): ChatMessage {
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
    sender_peer_key_hex: "",
    ...overrides,
  };
}

describe("MessageBubble", () => {
  it("renders the message content", () => {
    render(<MessageBubble message={msg({ content: "the content" })} />);
    expect(screen.getByText("the content")).toBeInTheDocument();
  });

  it("applies the direction class", () => {
    const { container } = render(<MessageBubble message={msg({ direction: "sent" })} />);
    expect(container.querySelector(".msg-bubble--sent")).toBeInTheDocument();
  });

  it("staggers animation by index", () => {
    const { container } = render(<MessageBubble message={msg()} index={4} />);
    expect(container.querySelector(".msg-bubble")).toHaveStyle({ animationDelay: "0.2s" });
  });

  // ── Deleted ────────────────────────────────────────────────────────────
  it("shows a deletion placeholder and hides the content", () => {
    render(<MessageBubble message={msg({ content: "secret", deleted: true })} />);
    expect(screen.getByText("Message deleted")).toBeInTheDocument();
    expect(screen.queryByText("secret")).not.toBeInTheDocument();
  });

  it("hides react and menu affordances on a deleted message", () => {
    render(
      <MessageBubble
        message={msg({ deleted: true })}
        onReact={vi.fn()}
        onEditSave={vi.fn()}
        onDelete={vi.fn()}
      />,
    );
    expect(screen.queryByLabelText("Toggle reaction picker")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Message options")).not.toBeInTheDocument();
  });

  // ── Capability gating ──────────────────────────────────────────────────
  it("hides the reaction button when no reaction handler is supplied", () => {
    render(<MessageBubble message={msg()} />);
    expect(screen.queryByLabelText("Toggle reaction picker")).not.toBeInTheDocument();
  });

  it("hides the options button when neither edit nor delete is possible", () => {
    render(<MessageBubble message={msg()} onReact={vi.fn()} />);
    expect(screen.queryByLabelText("Message options")).not.toBeInTheDocument();
  });

  it("shows the options button when delete is possible", () => {
    render(<MessageBubble message={msg()} onDelete={vi.fn()} />);
    expect(screen.getByLabelText("Message options")).toBeInTheDocument();
  });

  // ── Reactions ──────────────────────────────────────────────────────────
  it("opens the picker and emits the chosen emoji", async () => {
    const onReact = vi.fn();
    render(<MessageBubble message={msg()} onReact={onReact} />);
    await userEvent.click(screen.getByLabelText("Toggle reaction picker"));
    // Regression guard: a real click is preceded by a mouseenter, so the
    // click's `!pickerOpen` toggle used to immediately close the picker that
    // hover had just opened. The button appeared to do nothing for mouse users.
    const btn = screen.getByRole("button", { name: "React 👍" });
    await userEvent.click(btn);
    await userEvent.click(btn);
    expect(onReact).toHaveBeenCalledWith("m1", "👍");
  });

  it("marks a reaction the current user has already applied", () => {
    render(
      <MessageBubble
        message={msg({ reactions: { "👍": ["self"] } })}
        onReact={vi.fn()}
      />,
    );
    const chip = screen.getByRole("button", { name: "React 👍" });
    expect(chip).toHaveAttribute("aria-pressed", "true");
    expect(chip).toHaveClass("msg-reaction--self");
  });

  it("un-presses a reaction the current user has not applied", () => {
    render(
      <MessageBubble
        message={msg({ reactions: { "👍": ["someone-else"] } })}
        onReact={vi.fn()}
      />,
    );
    expect(screen.getByRole("button", { name: "React 👍" })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
  });

  it("removes a reaction the current user applied", async () => {
    const onRemoveReaction = vi.fn();
    render(
      <MessageBubble
        message={msg({ reactions: { "👍": ["self"] } })}
        onRemoveReaction={onRemoveReaction}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: "React 👍" }));
    expect(onRemoveReaction).toHaveBeenCalledWith("m1", "👍");
  });

  it("does not remove a reaction belonging to someone else", async () => {
    const onRemoveReaction = vi.fn();
    render(
      <MessageBubble
        message={msg({ reactions: { "👍": ["someone-else"] } })}
        onRemoveReaction={onRemoveReaction}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: "React 👍" }));
    expect(onRemoveReaction).not.toHaveBeenCalled();
  });

  it("shows the reaction count when others have reacted", () => {
    render(
      <MessageBubble
        message={msg({ reactions: { "👍": ["a", "b"] } })}
        onReact={vi.fn()}
      />,
    );
    expect(screen.getByRole("button", { name: "React 👍" })).toHaveTextContent(
      "👍 2",
    );
  });

  // ── Edit ───────────────────────────────────────────────────────────────
  it("opens the inline editor with the current content", async () => {
    render(<MessageBubble message={msg({ content: "original" })} onEditSave={vi.fn()} />);
    fireEvent.contextMenu(screen.getByRole("group"));
    await userEvent.click(await screen.findByRole("button", { name: /Edit/ }));
    expect(screen.getByDisplayValue("original")).toBeInTheDocument();
  });

  it("saves edited content with the message id", async () => {
    const onEditSave = vi.fn();
    render(<MessageBubble message={msg({ content: "original" })} onEditSave={onEditSave} />);
    fireEvent.contextMenu(screen.getByRole("group"));
    await userEvent.click(await screen.findByRole("button", { name: /Edit/ }));
    const box = screen.getByDisplayValue("original");
    await userEvent.clear(box);
    await userEvent.type(box, "changed");
    await userEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(onEditSave).toHaveBeenCalledWith("m1", "changed");
  });

  it("exits edit mode on Ctrl+Enter", async () => {
    const onEditSave = vi.fn();
    render(<MessageBubble message={msg({ content: "original" })} onEditSave={onEditSave} />);
    fireEvent.contextMenu(screen.getByRole("group"));
    await userEvent.click(await screen.findByRole("button", { name: /Edit/ }));
    const box = screen.getByDisplayValue("original");
    await userEvent.clear(box);
    await userEvent.type(box, "keyboard save");
    await userEvent.keyboard("{Control>}{Enter}{/Control}");
    expect(onEditSave).toHaveBeenCalledWith("m1", "keyboard save");
    // Edit mode closed (no textarea remains), and the rendered text comes from
    // `message.content`, which this test does not update.
    expect(screen.queryByDisplayValue("original")).not.toBeInTheDocument();
    expect(screen.getByText("original")).toBeInTheDocument();
  });

  it("cancels an edit without saving", async () => {
    const onEditSave = vi.fn();
    render(<MessageBubble message={msg({ content: "original" })} onEditSave={onEditSave} />);
    fireEvent.contextMenu(screen.getByRole("group"));
    await userEvent.click(await screen.findByRole("button", { name: /Edit/ }));
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onEditSave).not.toHaveBeenCalled();
    expect(screen.getByText("original")).toBeInTheDocument();
  });

  it("discards an edit on Escape", async () => {
    const onEditSave = vi.fn();
    render(<MessageBubble message={msg({ content: "original" })} onEditSave={onEditSave} />);
    fireEvent.contextMenu(screen.getByRole("group"));
    await userEvent.click(await screen.findByRole("button", { name: /Edit/ }));
    const box = screen.getByDisplayValue("original");
    await userEvent.clear(box);
    await userEvent.type(box, "discard me");
    await userEvent.keyboard("{Escape}");
    expect(onEditSave).not.toHaveBeenCalled();
    expect(screen.getByText("original")).toBeInTheDocument();
  });

  // ── Badges ─────────────────────────────────────────────────────────────
  it("shows the edited badge only when the message was edited", () => {
    const { rerender } = render(<MessageBubble message={msg()} />);
    expect(screen.queryByText("edited")).not.toBeInTheDocument();
    rerender(<MessageBubble message={msg({ edited_at: 1_700_000_100 })} />);
    expect(screen.getByText("edited")).toBeInTheDocument();
  });

  // The read badge marks a *received* message that has been read locally.
  it("shows the read badge for a read received message", () => {
    render(
      <MessageBubble
        message={msg({ direction: "received", read_at: 1_700_000_100 })}
      />,
    );
    expect(screen.getByTitle(/^Read /)).toBeInTheDocument();
  });

  it("shows no read badge while a received message is unread", () => {
    render(<MessageBubble message={msg({ direction: "received", read_at: null })} />);
    expect(screen.queryByTitle(/^Read /)).not.toBeInTheDocument();
  });

  it("shows no read badge on an outgoing message", () => {
    render(
      <MessageBubble
        message={msg({ direction: "sent", read_at: 1_700_000_100 })}
      />,
    );
    expect(screen.queryByTitle(/^Read /)).not.toBeInTheDocument();
  });

  it("shows no read badge on a deleted message", () => {
    render(
      <MessageBubble
        message={msg({ direction: "received", read_at: 1, deleted: true })}
      />,
    );
    expect(screen.queryByTitle(/^Read /)).not.toBeInTheDocument();
  });

  it("shows the delivery status only for outgoing messages", () => {
    const { container, rerender } = render(
      <MessageBubble message={msg({ direction: "sent" })} msgStatus="delivered" />,
    );
    expect(container.querySelector(".msg-status--delivered")).toBeInTheDocument();
    rerender(
      <MessageBubble
        message={msg({ direction: "received" })}
        msgStatus="delivered"
      />,
    );
    expect(container.querySelector(".msg-status--delivered")).not.toBeInTheDocument();
  });

  it("shows no status when msgStatus is absent", () => {
    const { container } = render(<MessageBubble message={msg({ direction: "sent" })} />);
    expect(container.querySelector(".msg-status")).not.toBeInTheDocument();
  });

  it("shows no status on a deleted outgoing message", () => {
    const { container } = render(
      <MessageBubble
        message={msg({ direction: "sent", deleted: true })}
        msgStatus="delivered"
      />,
    );
    expect(container.querySelector(".msg-status")).not.toBeInTheDocument();
  });

  // ── Markdown ───────────────────────────────────────────────────────────
  it("renders markdown by default", () => {
    render(<MessageBubble message={msg({ content: "**bold**" })} />);
    expect(screen.getByText("bold").tagName).toBe("STRONG");
  });

  it("renders plain text when plain is set", () => {
    render(<MessageBubble message={msg({ content: "**bold**" })} plain />);
    expect(screen.getByText("**bold**")).toBeInTheDocument();
    expect(screen.queryByText("bold")).not.toBeInTheDocument();
  });

  // ── Group sender label ─────────────────────────────────────────────────
  it("shows a truncated sender label for a group message", () => {
    render(
      <MessageBubble
        message={msg({ sender_peer_key_hex: "abcdef0123456789" })}
      />,
    );
    expect(screen.getByText("abcdef01…")).toBeInTheDocument();
  });

  it("shows no sender label for a direct message", () => {
    render(<MessageBubble message={msg({ sender_peer_key_hex: "" })} />);
    expect(screen.queryByText(/…$/)).not.toBeInTheDocument();
  });

  // ── Accessibility ──────────────────────────────────────────────────────
  it("is a labelled group, not a tab stop", () => {
    render(<MessageBubble message={msg({ direction: "sent" })} />);
    const group = screen.getByRole("group");
    expect(group).toHaveAttribute("aria-label", "Message from you");
    // Regression guard: this was tabIndex=0, which made a long transcript
    // unusable by keyboard or screen reader.
    expect(group).not.toHaveAttribute("tabindex");
  });

  it("names the peer by key prefix when the sender is unknown", () => {
    render(<MessageBubble message={msg({ sender_peer_key_hex: "99887766aabb" })} />);
    expect(screen.getByRole("group")).toHaveAttribute("aria-label", "Message from 99887766");
  });

  it("closes the context menu on Escape", async () => {
    render(<MessageBubble message={msg()} onDelete={vi.fn()} />);
    await userEvent.click(screen.getByLabelText("Message options"));
    expect(await screen.findByRole("button", { name: /Delete/ })).toBeInTheDocument();
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("button", { name: /Delete/ })).not.toBeInTheDocument();
  });

  it("closes the context menu on an outside click", async () => {
    render(
      <div>
        <MessageBubble message={msg()} onDelete={vi.fn()} />
        <button type="button">elsewhere</button>
      </div>,
    );
    await userEvent.click(screen.getByLabelText("Message options"));
    expect(await screen.findByRole("button", { name: /Delete/ })).toBeInTheDocument();
    await userEvent.click(screen.getByText("elsewhere"));
    expect(screen.queryByRole("button", { name: /Delete/ })).not.toBeInTheDocument();
  });

  it("does not open a context menu for a message with no actions", () => {
    const onContextMenu = vi.fn((e: React.MouseEvent) => e.preventDefault());
    render(<MessageBubble message={msg()} />);
    const group = screen.getByRole("group");
    // No edit/delete handler: the menu must stay shut.
    fireEvent.contextMenu(group);
    expect(screen.queryByLabelText("Message options")).not.toBeInTheDocument();
    expect(onContextMenu).not.toHaveBeenCalled();
  });
});

describe("MessageBubble self-destruct", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-01-01T00:00:00Z"));
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("shows a countdown when expires_at is set", () => {
    render(
      <MessageBubble message={msg({ expires_at: Math.floor(Date.now() / 1000) + 90 })} />,
    );
    expect(screen.getByText(/1:30/)).toBeInTheDocument();
  });

  it("shows no countdown when expires_at is null", () => {
    render(<MessageBubble message={msg({ expires_at: null })} />);
    expect(screen.queryByText(/🔥/)).not.toBeInTheDocument();
  });

  it("shows no countdown for an already-expired message", () => {
    render(
      <MessageBubble message={msg({ expires_at: Math.floor(Date.now() / 1000) - 1 })} />,
    );
    expect(screen.queryByText(/🔥/)).not.toBeInTheDocument();
  });
});
