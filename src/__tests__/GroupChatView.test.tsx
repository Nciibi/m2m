import { describe, it, expect, vi, beforeEach } from "vitest";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { MockEventHandler } from "./tauriMock";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((name: string, handler: MockEventHandler) => {
    eventHandlers.set(name, handler);
    return Promise.resolve(() => eventHandlers.delete(name));
  }),
}));

// `vi.hoisted` callbacks are hoisted above the import statements, so they cannot
// reference an imported *value*. The Map is therefore built inline; the
// `MockEventHandler` type import is erased and so is safe to use here.
const { eventHandlers } = vi.hoisted(() => ({
  eventHandlers: new Map<string, MockEventHandler>(),
}));

const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => mockInvoke(...a) }));

const addToast = vi.fn();
const setView = vi.fn();
vi.mock("../context/AppContext", () => ({
  useApp: () => ({
    toasts: [],
    removeToast: vi.fn(),
    addToast,
    setView,
    identity: null,
  }),
}));

import GroupChatView from "../views/GroupChatView";

const KEY_A = "a".repeat(64);
const KEY_B = "b".repeat(64);

const GROUPS = [
  { group_id: "g1", group_name: "Journalists", member_count: 3, created_at: 1 },
  { group_id: "g2", group_name: "Family", member_count: 2, created_at: 2 },
];

const GROUP_DETAIL = {
  group_id: "g1",
  group_name: "Journalists",
  member_count: 3,
  created_at: 1,
  our_role: "admin",
  members: [],
};

/** A complete ChatMessage, as the backend returns it. */
function backendMessage(over: Record<string, unknown> = {}) {
  return {
    id: "m1",
    content: "hello group",
    direction: "sent",
    timestamp: 1_700_000_000,
    read_at: null,
    edited_at: null,
    deleted: false,
    expires_at: null,
    reactions: {},
    sender_peer_key_hex: KEY_A,
    ...over,
  };
}

/** Deliver a payload to a registered listener, as Tauri does. */
function emit(name: string, payload: unknown) {
  eventHandlers.get(name)?.({ event: name, id: 0, payload });
}

beforeEach(() => {
  vi.clearAllMocks();
  eventHandlers.clear();
  mockInvoke.mockImplementation((cmd: string) => {
    switch (cmd) {
      case "list_groups":
        return Promise.resolve(GROUPS);
      case "get_group_info":
        return Promise.resolve(GROUP_DETAIL);
      case "load_group_messages":
        return Promise.resolve([]);
      default:
        return Promise.resolve(null);
    }
  });
});

describe("GroupChatView — list", () => {
  it("lists the groups returned by the backend", async () => {
    render(<GroupChatView />);
    expect(await screen.findByText("Journalists")).toBeInTheDocument();
    expect(screen.getByText("Family")).toBeInTheDocument();
  });

  it("shows the member count for each group", async () => {
    render(<GroupChatView />);
    expect(await screen.findByText("3 members")).toBeInTheDocument();
    expect(screen.getByText("2 members")).toBeInTheDocument();
  });

  it("shows an empty state when there are no groups", async () => {
    mockInvoke.mockImplementation((cmd: string) =>
      Promise.resolve(cmd === "list_groups" ? [] : null),
    );
    render(<GroupChatView />);
    expect(await screen.findByText("No groups yet")).toBeInTheDocument();
  });

  // A failing `list_groups` used to leave the user staring at a permanent
  // "No groups yet" with no indication anything went wrong.
  it("does not show the empty state when loading fails", async () => {
    mockInvoke.mockImplementation((cmd: string) =>
      cmd === "list_groups" ? Promise.reject("db locked") : Promise.resolve(null),
    );
    render(<GroupChatView />);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("list_groups"));
    expect(screen.queryByText("No groups yet")).not.toBeInTheDocument();
  });

  it("opens a group and loads its messages", async () => {
    render(<GroupChatView />);
    await userEvent.click(await screen.findByText("Journalists"));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("load_group_messages", {
        groupId: "g1",
        limit: 100,
      }),
    );
  });

  it("shows the open group's name, member count and role", async () => {
    render(<GroupChatView />);
    await userEvent.click(await screen.findByText("Journalists"));
    expect(await screen.findByText(/3 members/)).toBeInTheDocument();
    expect(screen.getByText(/admin/)).toBeInTheDocument();
  });

  it("returns to the group list from an open group", async () => {
    render(<GroupChatView />);
    await userEvent.click(await screen.findByText("Journalists"));
    await userEvent.click(backToGroupsButton());
    expect(await screen.findByText("Family")).toBeInTheDocument();
  });

  it("clears loaded messages when returning to the list", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_groups") return Promise.resolve(GROUPS);
      if (cmd === "get_group_info") return Promise.resolve(GROUP_DETAIL);
      if (cmd === "load_group_messages")
        return Promise.resolve([backendMessage({ id: "loaded" })]);
      return Promise.resolve(null);
    });
    render(<GroupChatView />);
    await userEvent.click(await screen.findByText("Journalists"));
    expect(await screen.findByText("hello group")).toBeInTheDocument();
    await userEvent.click(backToGroupsButton());
    expect(screen.queryByText("hello group")).not.toBeInTheDocument();
  });
});

describe("GroupChatView — group events", () => {
  it("refreshes the list on a group event", async () => {
    render(<GroupChatView />);
    await screen.findByText("Journalists");
    mockInvoke.mockClear();
    emit("m2m://group-event", {
      group_id: "g1",
      event_type: "member_joined",
      peer_key_hex: KEY_A,
    });
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("list_groups"));
  });

  // An unrecognised event_type means the payload could not be understood, so
  // the list must still be refreshed — just without trusting the label.
  it("refreshes even when the event type is unknown", async () => {
    render(<GroupChatView />);
    await screen.findByText("Journalists");
    mockInvoke.mockClear();
    emit("m2m://group-event", {
      group_id: "g1",
      event_type: "something_new_in_5_0",
      peer_key_hex: KEY_A,
    });
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("list_groups"));
  });

  it("refreshes on a malformed event rather than trusting it", async () => {
    render(<GroupChatView />);
    await screen.findByText("Journalists");
    mockInvoke.mockClear();
    emit("m2m://group-event", { group_id: 12345, event_type: "member_joined" });
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("list_groups"));
  });

  it("does not throw on a non-object event payload", async () => {
    render(<GroupChatView />);
    await screen.findByText("Journalists");
    expect(() => emit("m2m://group-event", "nonsense")).not.toThrow();
    expect(() => emit("m2m://group-event", null)).not.toThrow();
  });
});

describe("GroupChatView — inbound group messages", () => {
  async function openGroup() {
    render(<GroupChatView />);
    await userEvent.click(await screen.findByText("Journalists"));
    await screen.findByRole("textbox");
  }

  it("appends an inbound group message", async () => {
    await openGroup();
    emit("m2m://group-message", {
      group_id: "g1",
      message: backendMessage({ id: "in1", content: "inbound text" }),
    });
    expect(await screen.findByText("inbound text")).toBeInTheDocument();
  });

  // The emitter supplies `direction`, but the bubble renders from the flag the
  // view sets, so it is forced to "received" for both paths.
  it("renders an inbound message as received regardless of the payload flag", async () => {
    await openGroup();
    emit("m2m://group-message", {
      group_id: "g1",
      message: backendMessage({ id: "in2", direction: "sent", content: "flagged sent" }),
    });
    const bubble = (await screen.findByText("flagged sent")).closest(".msg-bubble");
    expect(bubble).toHaveClass("msg-bubble--received");
    expect(bubble).not.toHaveClass("msg-bubble--sent");
  });

  it("preserves message order across several events", async () => {
    await openGroup();
    // `act` matters here: three synchronous emits each schedule a functional
    // state update, and without it the assertions run before React commits.
    act(() => {
      for (const n of [1, 2, 3]) {
        emit("m2m://group-message", {
          group_id: "g1",
          message: backendMessage({ id: `i${n}`, content: `msg ${n}` }),
        });
      }
    });
    const bubbles = Array.from(
      document.querySelectorAll(".msg-area .msg-content"),
    ).map((el) => el.textContent);
    expect(bubbles).toEqual(["msg 1", "msg 2", "msg 3"]);
  });

  // The validator is the security boundary for peer-controlled content.
  it("drops a malformed payload instead of rendering it", async () => {
    await openGroup();
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    for (const bad of [
      null,
      "string payload",
      { group_id: "g1" },
      { group_id: "g1", message: null },
      { group_id: "g1", message: { ...backendMessage(), sender_peer_key_hex: 42 } },
      { group_id: "g1", message: { ...backendMessage(), content: { evil: true } } },
    ]) {
      emit("m2m://group-message", bad);
    }
    expect(screen.queryByText("hello group")).not.toBeInTheDocument();
    expect(screen.getByText("No messages yet")).toBeInTheDocument();
    expect(warn).toHaveBeenCalled();
    warn.mockRestore();
  });

  it("keeps already-loaded messages when a bad payload arrives", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_groups") return Promise.resolve(GROUPS);
      if (cmd === "get_group_info") return Promise.resolve(GROUP_DETAIL);
      if (cmd === "load_group_messages")
        return Promise.resolve([backendMessage({ id: "keep", content: "keep me" })]);
      return Promise.resolve(null);
    });
    await openGroup();
    expect(await screen.findByText("keep me")).toBeInTheDocument();
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    emit("m2m://group-message", { group_id: "g1" });
    expect(screen.getByText("keep me")).toBeInTheDocument();
    warn.mockRestore();
  });

  it("unregisters both listeners on unmount", async () => {
    const { unmount } = render(<GroupChatView />);
    await screen.findByText("Journalists");
    expect(eventHandlers.has("m2m://group-event")).toBe(true);
    expect(eventHandlers.has("m2m://group-message")).toBe(true);
    unmount();
    await waitFor(() => expect(eventHandlers.size).toBe(0));
  });
});

describe("GroupChatView — create group", () => {
  it("opens the create form", async () => {
    render(<GroupChatView />);
    await userEvent.click(await screen.findByRole("button", { name: /New Group/ }));
    expect(await screen.findByPlaceholderText("My Group")).toBeInTheDocument();
  });

  it("does nothing without a group name", async () => {
    render(<GroupChatView />);
    await userEvent.click(await screen.findByRole("button", { name: /New Group/ }));
    await userEvent.type(screen.getByPlaceholderText(/aabbccdd/), KEY_A);
    await userEvent.click(screen.getByRole("button", { name: "Create Group" }));
    expect(mockInvoke).not.toHaveBeenCalledWith("create_group", expect.anything());
  });

  // Keys that are not 64 hex chars are dropped rather than sent to the backend.
  it("rejects a member key that is not 64 characters", async () => {
    render(<GroupChatView />);
    await userEvent.click(await screen.findByRole("button", { name: /New Group/ }));
    await userEvent.type(screen.getByPlaceholderText("My Group"), "Team");
    await userEvent.type(screen.getByPlaceholderText(/aabbccdd/), "tooshort");
    await userEvent.click(screen.getByRole("button", { name: "Create Group" }));
    await waitFor(() =>
      expect(addToast).toHaveBeenCalledWith(
        expect.stringContaining("at least one member"),
        "error",
      ),
    );
  });

  it("creates a group with the trimmed name and valid member keys", async () => {
    const created = { group_id: "g9", group_name: "Team", member_count: 1, created_at: 9 };
    mockInvoke.mockImplementation((cmd: string) =>
      cmd === "list_groups"
        ? Promise.resolve(GROUPS)
        : cmd === "create_group"
          ? Promise.resolve(created)
          : Promise.resolve(null),
    );
    render(<GroupChatView />);
    await userEvent.click(await screen.findByRole("button", { name: /New Group/ }));
    await userEvent.type(screen.getByPlaceholderText("My Group"), "  Team  ");
    await userEvent.type(
      screen.getByPlaceholderText(/aabbccdd/),
      `${KEY_A}, , not-a-key , ${KEY_B}`,
    );
    await userEvent.click(screen.getByRole("button", { name: "Create Group" }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("create_group", {
        groupName: "Team",
        memberPeerKeys: [KEY_A, KEY_B],
      }),
    );
  });

  it("appends the new group to the list and closes the form", async () => {
    const created = { group_id: "g9", group_name: "Team", member_count: 1, created_at: 9 };
    mockInvoke.mockImplementation((cmd: string) =>
      cmd === "list_groups"
        ? Promise.resolve(GROUPS)
        : cmd === "create_group"
          ? Promise.resolve(created)
          : Promise.resolve(null),
    );
    render(<GroupChatView />);
    await userEvent.click(await screen.findByRole("button", { name: /New Group/ }));
    await userEvent.type(screen.getByPlaceholderText("My Group"), "Team");
    await userEvent.type(screen.getByPlaceholderText(/aabbccdd/), KEY_A);
    await userEvent.click(screen.getByRole("button", { name: "Create Group" }));
    expect(await screen.findByText("Team")).toBeInTheDocument();
    expect(screen.queryByPlaceholderText("My Group")).not.toBeInTheDocument();
    expect(addToast).toHaveBeenCalledWith("Group created!", "success");
  });

  it("surfaces a string rejection from the backend", async () => {
    mockInvoke.mockImplementation((cmd: string) =>
      cmd === "list_groups"
        ? Promise.resolve(GROUPS)
        : cmd === "create_group"
          ? Promise.reject("not authorised to create groups")
          : Promise.resolve(null),
    );
    render(<GroupChatView />);
    await userEvent.click(await screen.findByRole("button", { name: /New Group/ }));
    await userEvent.type(screen.getByPlaceholderText("My Group"), "Team");
    await userEvent.type(screen.getByPlaceholderText(/aabbccdd/), KEY_A);
    await userEvent.click(screen.getByRole("button", { name: "Create Group" }));
    await waitFor(() =>
      expect(addToast).toHaveBeenCalledWith(
        expect.stringContaining("not authorised"),
        "error",
      ),
    );
    // The form must stay open so the input is not lost.
    expect(screen.getByPlaceholderText("My Group")).toBeInTheDocument();
  });

  it("surfaces an AppError-shaped rejection without rendering [object Object]", async () => {
    // Commands reject with `{code, message}` since the error taxonomy landed.
    // Concatenating that with a string yields "[object Object]" — invisible to
    // `tsc`, because `invoke<T>` does not type its rejection value, and
    // therefore only catchable by a test that actually rejects with the real
    // shape.
    mockInvoke.mockImplementation((cmd: string) =>
      cmd === "list_groups"
        ? Promise.resolve(GROUPS)
        : cmd === "create_group"
          ? Promise.reject({
              code: "network.io",
              message: "not authorised to create groups",
            })
          : Promise.resolve(null),
    );
    render(<GroupChatView />);
    await userEvent.click(await screen.findByRole("button", { name: /New Group/ }));
    await userEvent.type(screen.getByPlaceholderText("My Group"), "Team");
    await userEvent.type(screen.getByPlaceholderText(/aabbccdd/), KEY_A);
    await userEvent.click(screen.getByRole("button", { name: "Create Group" }));
    await waitFor(() =>
      expect(addToast).toHaveBeenCalledWith(
        expect.stringContaining("not authorised"),
        "error",
      ),
    );
    expect(addToast).not.toHaveBeenCalledWith(
      expect.stringContaining("[object Object]"),
      expect.anything(),
    );
  });
});

describe("GroupChatView — sending", () => {
  async function openGroup() {
    render(<GroupChatView />);
    await userEvent.click(await screen.findByText("Journalists"));
    await screen.findByRole("textbox");
  }

  it("disables send while the composer is empty", async () => {
    await openGroup();
    expect(screen.getByRole("button", { name: /send/i })).toBeDisabled();
  });

  it("enables send once text is typed", async () => {
    await openGroup();
    await userEvent.type(screen.getByRole("textbox"), "hi");
    expect(screen.getByRole("button", { name: /send/i })).toBeEnabled();
  });

  it("trims the message and clears the composer on success", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_groups") return Promise.resolve(GROUPS);
      if (cmd === "get_group_info") return Promise.resolve(GROUP_DETAIL);
      if (cmd === "load_group_messages") return Promise.resolve([]);
      if (cmd === "send_group_message")
        return Promise.resolve(backendMessage({ id: "s1", content: "hi", direction: "sent" }));
      return Promise.resolve(null);
    });
    await openGroup();
    await userEvent.type(screen.getByRole("textbox"), "   hi   ");
    await userEvent.click(screen.getByRole("button", { name: /send/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("send_group_message", {
        groupId: "g1",
        content: "hi",
      }),
    );
    expect(await screen.findByText("hi")).toBeInTheDocument();
    expect(screen.getByRole("textbox")).toHaveValue("");
  });

  it("keeps the text when sending fails", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_groups") return Promise.resolve(GROUPS);
      if (cmd === "get_group_info") return Promise.resolve(GROUP_DETAIL);
      if (cmd === "load_group_messages") return Promise.resolve([]);
      if (cmd === "send_group_message") return Promise.reject("group key rotated");
      return Promise.resolve(null);
    });
    await openGroup();
    await userEvent.type(screen.getByRole("textbox"), "keep this");
    await userEvent.click(screen.getByRole("button", { name: /send/i }));
    await waitFor(() =>
      expect(addToast).toHaveBeenCalledWith(
        expect.stringContaining("group key rotated"),
        "error",
      ),
    );
    // Losing the draft on a transient failure is the worst outcome here.
    expect(screen.getByRole("textbox")).toHaveValue("keep this");
  });

  it("sends on Enter", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_groups") return Promise.resolve(GROUPS);
      if (cmd === "get_group_info") return Promise.resolve(GROUP_DETAIL);
      if (cmd === "load_group_messages") return Promise.resolve([]);
      if (cmd === "send_group_message")
        return Promise.resolve(backendMessage({ id: "s2", content: "enter sent" }));
      return Promise.resolve(null);
    });
    await openGroup();
    await userEvent.type(screen.getByRole("textbox"), "enter sent{Enter}");
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("send_group_message", {
        groupId: "g1",
        content: "enter sent",
      }),
    );
  });

  it("inserts a newline on Shift+Enter instead of sending", async () => {
    await openGroup();
    await userEvent.type(screen.getByRole("textbox"), "line one{Shift>}{Enter}{/Shift}");
    expect(mockInvoke).not.toHaveBeenCalledWith("send_group_message", expect.anything());
  });

  it("does not send an all-whitespace message", async () => {
    await openGroup();
    await userEvent.type(screen.getByRole("textbox"), "   ");
    expect(screen.getByRole("button", { name: /send/i })).toBeDisabled();
  });
});

/**
 * The "back to groups" header button.
 *
 * Scoped to the header because `Sidebar` renders its own nav button whose
 * accessible name also matches /Groups/.
 */
function backToGroupsButton(): HTMLElement {
  const header = document.querySelector(".app-header");
  const btn = header?.querySelector("button");
  if (!btn) throw new Error("header back button not found");
  return btn as HTMLElement;
}
