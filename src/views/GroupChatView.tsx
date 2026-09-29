import { errorMessage } from "../utils";
import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { asArray, asGroupEvent, asGroupMessageEvent } from "../events";
import { Button, Badge, Input, ToastContainer } from "../components/ui";
import {
  ArrowLeftIcon, PlusIcon, GroupsIcon, MessageIcon, SendIcon, LockIcon,
} from "../components/ui/Icons";
import Sidebar from "../components/Sidebar";
import MessageBubble from "../components/chat/MessageBubble";
import { useApp } from "../context/AppContext";
import type { GroupInfo, GroupDetail, ChatMessage } from "../types";

export default function GroupChatView() {
  const { toasts, removeToast, addToast, setView } = useApp();
  const [groups, setGroups] = useState<GroupInfo[]>([]);
  const [activeGroup, setActiveGroup] = useState<GroupDetail | null>(null);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [text, setText] = useState("");
  const [sending, setSending] = useState(false);
  const [showCreate, setShowCreate] = useState(false);
  const [createName, setCreateName] = useState("");
  const [createMembers, setCreateMembers] = useState("");
  // Distinguishes "the backend says I have no groups" from "the load failed".
  // Both leave `groups` empty, so without this the user is shown "No groups yet"
  // when the store is in fact locked or unreadable — which reads as data loss
  // rather than as an error, and there is no way to tell the two apart.
  const [loadFailed, setLoadFailed] = useState(false);

  const loadGroups = useCallback(async () => {
    try {
      // `asArray`: `invoke<GroupInfo[]>` asserts the shape but does not check
      // it, and `groups.map(...)` in the render below would throw on `null`.
      setGroups(asArray<GroupInfo>(await invoke("list_groups")));
      setLoadFailed(false);
    } catch (e) {
      setLoadFailed(true);
      addToast("Could not load groups: " + errorMessage(e), "error");
    }
  }, [addToast]);

  const loadMessages = useCallback(async (groupId: string) => {
    try {
      setMessages(asArray<ChatMessage>(await invoke("load_group_messages", { groupId, limit: 100 })));
    } catch { /* noop */ }
  }, []);

  useEffect(() => {
    loadGroups();
    const unlisten = listen("m2m://group-event", (event) => {
      // `event_type` is a known set; an unknown one means we do not understand
      // what changed, so refresh anyway but do not trust the label.
      if (!asGroupEvent(event.payload)) {
        console.warn("M2M: unrecognised m2m://group-event payload");
      }
      loadGroups();
    });
    const unlistenMsg = listen("m2m://group-message", (event) => {
      // The message body is peer-controlled and reaches renderMarkdown, and
      // `sender_peer_key_hex` is rendered as the sender label.
      const payload = asGroupMessageEvent(event.payload);
      if (!payload) {
        console.warn("M2M: dropping malformed m2m://group-message payload");
        return;
      }
      // `direction` is forced to "received" for BOTH the locally-sent and the
      // inbound event (the emitter supplies it, but the bubble renders from
      // this flag), so keep the previous behaviour explicitly rather than
      // silently depending on the payload.
      const msg: ChatMessage = { ...payload.message, direction: "received" };

      // Only append when the message is for the group on screen. The group id
      // used to be destructured and then discarded, so a message arriving for
      // any *other* group was appended to the open group's transcript — the
      // same cross-conversation contamination the 1:1 path had.
      setActiveGroupRef((active) => {
        if (active && active.group_id !== payload.group_id) return active;
        setMessages((prev) => [...prev, msg]);
        return active;
      });
    });
    return () => {
      unlisten.then((f) => f());
      unlistenMsg.then((f) => f());
    };
  }, [loadGroups]);

  const handleCreateGroup = async () => {
    if (!createName.trim()) return;
    const members = createMembers
      .split(",")
      .map((m) => m.trim())
      .filter((m) => m.length === 64);
    if (members.length === 0) {
      addToast("Add at least one member (64-char hex key)", "error");
      return;
    }
    try {
      const info = await invoke<GroupInfo>("create_group", {
        groupName: createName.trim(),
        memberPeerKeys: members,
      });
      setGroups((prev) => [...prev, info]);
      setShowCreate(false);
      setCreateName("");
      setCreateMembers("");
      addToast("Group created!", "success");
    } catch (e) {
      addToast("Failed to create group: " + (errorMessage(e) || "unknown"), "error");
    }
  };

  const handleOpenGroup = async (groupId: string) => {
    try {
      const detail = await invoke<GroupDetail>("get_group_info", { groupId });
      setActiveGroup(detail);
      loadMessages(groupId);
    } catch { /* noop */ }
  };

  const handleSendMessage = async () => {
    if (!text.trim() || sending || !activeGroup) return;
    setSending(true);
    try {
      const msg = await invoke<ChatMessage>("send_group_message", {
        groupId: activeGroup.group_id,
        content: text.trim(),
      });
      setMessages((prev) => [...prev, msg]);
      setText("");
    } catch (e) {
      addToast("Failed to send: " + (errorMessage(e) || "unknown"), "error");
    } finally {
      setSending(false);
    }
  };

  return (
    <div className="app-shell">
      <Sidebar currentView="groups" onNavigate={setView} />
      <div className="app-main">
      <div className="app-header">
        <h1 className="app-header__title">
          <span className="app-header__icon-bg app-header__icon-bg--accent">
            <GroupsIcon size={18} color="var(--color-accent-bright)" />
          </span>
          {activeGroup ? activeGroup.group_name : "Group Chats"}
          {activeGroup && (
            <Badge variant="default" compact>
              {activeGroup.member_count} members · {activeGroup.our_role}
            </Badge>
          )}
        </h1>
        <div className="app-header__actions">
          {activeGroup ? (
            <Button variant="secondary" size="sm" onClick={() => { setActiveGroup(null); setMessages([]); }}>
              <ArrowLeftIcon size={16} /> Groups
            </Button>
          ) : (
            <>
              <Button variant="secondary" size="sm" onClick={() => setView("hub")}>
                <ArrowLeftIcon size={16} /> Hub
              </Button>
              <Button size="sm" onClick={() => setShowCreate(!showCreate)}>
                <PlusIcon size={16} /> New Group
              </Button>
            </>
          )}
        </div>
      </div>

      {/* CREATE GROUP FORM */}
      {showCreate && !activeGroup && (
        <div className="naming-panel">
          <label>Group Name <Input placeholder="My Group" value={createName} onChange={(e) => setCreateName(e.target.value)} compact /></label>
          <label>Member Peer Keys (comma-separated hex) <Input placeholder="aabbccdd…, eeff0011…" value={createMembers} onChange={(e) => setCreateMembers(e.target.value)} mono compact /></label>
          <Button onClick={handleCreateGroup}>Create Group</Button>
        </div>
      )}

      {/* GROUP LIST */}
      {!activeGroup && (
        <div className="conv-list">
          {groups.length === 0 ? (
            <div className="conv-empty">
              <GroupsIcon size={48} color="var(--color-text-muted)" />
              {loadFailed ? (
                <>
                  <p className="conv-empty__title">Could not load groups</p>
                  <p className="conv-empty__desc">
                    Your groups could not be read. This is not the same as having
                    no groups — retry before concluding anything.
                  </p>
                  <Button variant="secondary" size="sm" onClick={loadGroups}>
                    Retry
                  </Button>
                </>
              ) : (
                <>
                  <p className="conv-empty__title">No groups yet</p>
                  <p className="conv-empty__desc">Create a group to start an encrypted group conversation.</p>
                </>
              )}
            </div>
          ) : (
            groups.map((g) => (
              <button key={g.group_id} className="conv-item" onClick={() => handleOpenGroup(g.group_id)}>
                <div className="conv-avatar" style={{ background: "var(--color-accent-bright)" }}>
                  <GroupsIcon size={20} color="white" />
                </div>
                <div className="conv-body">
                  <div className="conv-top">
                    <span className="conv-name">{g.group_name}</span>
                    <span className="conv-time">{g.member_count} members</span>
                  </div>
                  <p className="conv-preview">Tap to open group chat</p>
                </div>
              </button>
            ))
          )}
        </div>
      )}

      {/* GROUP MESSAGES */}
      {activeGroup && (
        <>
          <div className="msg-area" id="group-message-list">
            {messages.length === 0 ? (
              <div className="conv-empty" style={{ marginTop: 'var(--space-2xl)' }}>
                <MessageIcon size={48} color="var(--color-text-muted)" />
                <p className="conv-empty__title">No messages yet</p>
                <p className="conv-empty__desc">Start the conversation!</p>
              </div>
            ) : (
              messages.map((m, i) => (
                <MessageBubble key={m.id} message={m} index={i} plain />
              ))
            )}
          </div>

          {/* INPUT */}
          <form className="msg-input-area" onSubmit={(e) => { e.preventDefault(); handleSendMessage(); }}>
            <div className="msg-input-wrap">
              <textarea
                id="group-message-input"
                value={text}
                onChange={(e) => setText(e.target.value)}
                onKeyDown={(e) => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); handleSendMessage(); } }}
                placeholder="Type a group message…"
                rows={1}
              />
            </div>
            <button
              type="submit"
              className="msg-send-btn"
              disabled={!text.trim() || sending}
              // Icon-only button: without this it has no accessible name, so a
              // screen reader announces nothing at all, and the busy state is
              // indistinguishable from the idle state.
              aria-label={sending ? "Sending" : "Send message"}
              aria-busy={sending}
            >
              {sending ? <span className="msg-send-spinner" /> : <SendIcon size={20} />}
            </button>
          </form>
        </>
      )}

      <div className="msg-footer">
        <span><LockIcon size={12} /> Group E2EE · Sender Keys</span>
        <span>Enter to send</span>
      </div>
      </div>
      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </div>
  );
}
