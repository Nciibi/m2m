import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Button, Badge, Input, ToastContainer } from "../components/ui";
import {
  ArrowLeftIcon, PlusIcon, GroupsIcon, MessageIcon, SendIcon, LockIcon,
} from "../components/ui/Icons";
import Sidebar from "../components/Sidebar";
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

  const loadGroups = useCallback(async () => {
    try { setGroups(await invoke<GroupInfo[]>("list_groups")); } catch { /* noop */ }
  }, []);

  const loadMessages = useCallback(async (groupId: string) => {
    try {
      const msgs = await invoke<ChatMessage[]>("load_group_messages", { groupId, limit: 100 });
      setMessages(msgs);
    } catch { /* noop */ }
  }, []);

  useEffect(() => {
    loadGroups();
    const unlisten = listen<any>("m2m://group-event", () => { loadGroups(); });
    const unlistenMsg = listen<any>("m2m://group-message", (event) => {
      const payload = event.payload.message || event.payload;
      const msg: ChatMessage = {
        id: payload.id || Date.now().toString(),
        content: payload.content || "",
        direction: "received",
        timestamp: payload.timestamp || Math.floor(Date.now() / 1000),
        read_at: null,
        edited_at: null,
        deleted: false,
        expires_at: null,
        reactions: {},
        sender_peer_key_hex: payload.sender_peer_key_hex || "",
      };
      setMessages((prev) => [...prev, msg]);
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
    } catch (e: any) {
      addToast("Failed to create group: " + (typeof e === "string" ? e : e?.message || "unknown"), "error");
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
    } catch (e: any) {
      addToast("Failed to send: " + (typeof e === "string" ? e : e?.message || "unknown"), "error");
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
              <p className="conv-empty__title">No groups yet</p>
              <p className="conv-empty__desc">Create a group to start an encrypted group conversation.</p>
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
                <div key={m.id} className={`msg-bubble msg-bubble--${m.direction}`} style={{ animationDelay: `${i * 0.05}s` }}>
                  {m.direction === "received" && m.sender_peer_key_hex && (
                    <div className="msg-sender-label">{m.sender_peer_key_hex.substring(0, 8)}…</div>
                  )}
                  <div className="msg-content">
                    {m.deleted ? <em style={{ opacity: 0.5, fontStyle: 'italic' }}>Deleted</em> : m.content}
                  </div>
                  <span className="msg-footer-row">
                    <span className="msg-time">
                      {new Date(m.timestamp * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
                    </span>
                  </span>
                </div>
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
            <button type="submit" className="msg-send-btn" disabled={!text.trim() || sending}>
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
