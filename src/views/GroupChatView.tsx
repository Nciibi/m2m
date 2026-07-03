import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ToastContainer } from "../components/ui";
import { useApp } from "../context/AppContext";
import type { GroupInfo, GroupDetail, GroupMember, ChatMessage } from "../types";

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
      const msg: ChatMessage = {
        id: event.payload.message_id || Date.now().toString(),
        content: event.payload.content || "",
        direction: "received",
        timestamp: Math.floor(Date.now() / 1000),
        read_at: null,
        edited_at: null,
        deleted: false,
        expires_at: null,
        reactions: {},
        sender_peer_key_hex: event.payload.sender_peer_key_hex || "",
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
      const info = await invoke<GroupDetail>("create_group", {
        groupName: createName.trim(),
        memberPeerKeys: members,
      });
      setGroups((prev) => [...prev, { group_id: info.group_id, group_name: info.group_name, member_count: info.member_count, created_at: info.created_at }]);
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
    <main className="premium-glass-card w-full h-full flex flex-col relative z-10">
      {/* HEADER */}
      <header className="h-[64px] min-h-[64px] flex justify-between items-center px-xl border-b border-border-subtle bg-surface/80 backdrop-blur-3xl shrink-0">
        <div className="flex items-center gap-md">
          <button onClick={() => { setActiveGroup(null); setView("hub"); }} className="text-on-surface-variant hover:text-primary transition-colors flex items-center p-xs hover:bg-bg-hover rounded-lg active:scale-95" title="Go back">
            <span className="material-symbols-outlined text-[22px]">arrow_back</span>
          </button>
          <div className="h-6 w-[1px] bg-border-subtle" />
          <div className="flex flex-col">
            <span className="font-bold text-on-surface text-body-lg leading-none">
              {activeGroup ? activeGroup.group_name : "Group Chats"}
            </span>
            {activeGroup && (
              <span className="font-label-xs text-[10px] text-text-muted uppercase tracking-wider">
                {activeGroup.member_count} members · {activeGroup.our_role}
              </span>
            )}
          </div>
        </div>
        {!activeGroup && (
          <button onClick={() => setShowCreate(!showCreate)} className="px-md py-1.5 bg-gradient-to-r from-primary to-inverse-primary text-white rounded-xl font-label-sm font-bold active:scale-95 transition-all shadow-[0_0_12px_rgba(99,102,241,0.3)]">
            <span className="material-symbols-outlined text-[18px]">add</span> New Group
          </button>
        )}
      </header>

      {/* CREATE GROUP FORM */}
      {showCreate && !activeGroup && (
        <div className="px-xl py-lg border-b border-border-subtle bg-primary/5">
          <div className="space-y-md">
            <div>
              <label className="font-label-xs text-[10px] text-text-muted uppercase tracking-wider">Group Name</label>
              <input
                value={createName}
                onChange={(e) => setCreateName(e.target.value)}
                className="w-full bg-input-bg border border-outline-variant rounded-lg px-md py-sm text-on-surface mt-xs"
                placeholder="My Group"
              />
            </div>
            <div>
              <label className="font-label-xs text-[10px] text-text-muted uppercase tracking-wider">Member Peer Keys (comma-separated hex)</label>
              <input
                value={createMembers}
                onChange={(e) => setCreateMembers(e.target.value)}
                className="w-full bg-input-bg border border-outline-variant rounded-lg px-md py-sm text-on-surface font-mono text-[12px] mt-xs"
                placeholder="aabbccdd..., eeff0011..."
              />
            </div>
            <button onClick={handleCreateGroup} className="w-full py-sm bg-gradient-to-r from-primary to-inverse-primary text-white rounded-xl font-label-sm font-bold active:scale-95 transition-all">
              Create Group
            </button>
          </div>
        </div>
      )}

      {/* GROUP LIST */}
      {!activeGroup && (
        <section className="flex-1 overflow-y-auto custom-scrollbar p-xl">
          {groups.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-4xl text-on-surface-variant gap-md">
              <span className="material-symbols-outlined text-5xl opacity-30">groups</span>
              <p className="font-headline-2xl text-headline-2xl text-on-surface">No groups yet</p>
              <p className="text-body-md max-w-[300px] text-center">Create a group to start an encrypted group conversation.</p>
            </div>
          ) : (
            <div className="space-y-sm">
              {groups.map((g) => (
                <button
                  key={g.group_id}
                  onClick={() => handleOpenGroup(g.group_id)}
                  className="w-full inner-glass p-md rounded-xl flex items-center gap-md hover:bg-bg-hover transition-all text-left"
                >
                  <div className="w-12 h-12 rounded-full bg-gradient-to-br from-primary to-secondary flex items-center justify-center shrink-0">
                    <span className="material-symbols-outlined text-white text-[20px]">group</span>
                  </div>
                  <div className="flex-1 min-w-0">
                    <div className="flex justify-between items-center mb-xs">
                      <span className="font-bold text-on-surface truncate">{g.group_name}</span>
                      <span className="text-label-xs text-text-muted shrink-0">{g.member_count} members</span>
                    </div>
                    <p className="text-body-base text-on-surface-variant truncate">Tap to open group chat</p>
                  </div>
                </button>
              ))}
            </div>
          )}
        </section>
      )}

      {/* GROUP MESSAGES */}
      {activeGroup && (
        <>
          <section className="flex-1 overflow-y-auto custom-scrollbar p-xl flex flex-col gap-xl">
            {messages.length === 0 ? (
              <div className="flex flex-col items-center justify-center flex-1 text-on-surface-variant gap-md">
                <span className="material-symbols-outlined text-5xl opacity-30">forum</span>
                <p className="font-body-md">No messages yet. Start the conversation!</p>
              </div>
            ) : (
              messages.map((m) => (
                <div key={m.id} className={`flex flex-col ${m.direction === "sent" ? "items-end" : "items-start"}`}>
                  <div className={`max-w-[75%] px-lg py-md rounded-2xl ${m.direction === "sent"
                    ? "bg-gradient-to-br from-primary to-inverse-primary text-white rounded-br-sm"
                    : "bg-input-bg backdrop-blur-xl border border-outline-variant"}`}>
                    {m.direction === "received" && (
                      <p className="font-bold text-[11px] text-primary mb-xs uppercase tracking-wider">
                        {m.sender_peer_key_hex?.substring(0, 8) || "unknown"}
                      </p>
                    )}
                    <p className="font-body-md whitespace-pre-wrap break-words">{m.deleted ? <em className="opacity-50">Deleted</em> : m.content}</p>
                  </div>
                  <span className="font-label-xs text-[10px] text-text-muted px-xs mt-xs">
                    {new Date(m.timestamp * 1000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                  </span>
                </div>
              ))
            )}
          </section>

          {/* INPUT */}
          <div className="p-xl border-t border-border-subtle input-blur shrink-0">
            <div className="flex items-center gap-md bg-input-bg rounded-2xl p-sm border border-outline-variant">
              <textarea
                value={text}
                onChange={(e) => setText(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); handleSendMessage(); }
                }}
                className="flex-1 bg-transparent border-none focus:ring-0 text-text-primary font-body-md py-md custom-scrollbar resize-none h-[48px]"
                placeholder="Type a group message..."
              />
              <button
                onClick={handleSendMessage}
                disabled={!text.trim() || sending}
                className="w-10 h-10 rounded-xl bg-gradient-to-tr from-primary to-inverse-primary text-white flex items-center justify-center disabled:opacity-50 transition-all active:scale-90 shrink-0"
              >
                <span className={`material-symbols-outlined text-[20px] ${sending ? 'animate-spin' : ''}`}>
                  {sending ? "sync" : "send"}
                </span>
              </button>
            </div>
          </div>
        </>
      )}

      {/* FOOTER */}
      <footer className="px-xl py-lg border-t border-border-subtle bg-surface-container-lowest/50 flex justify-between items-center shrink-0">
        <div className="flex items-center gap-sm">
          <span className="material-symbols-outlined text-sm text-tertiary">lock</span>
          <span className="font-mono-label text-[10px] text-text-muted uppercase tracking-widest">Group E2EE · Sender Keys</span>
        </div>
        <span className="font-mono-label text-[10px] text-text-muted">Enter to send</span>
      </footer>
      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </main>
  );
}
