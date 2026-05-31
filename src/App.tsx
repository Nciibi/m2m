import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./App.css";

interface IdentityInfo {
  fingerprint: string;
  public_key_hex: string;
  has_identity: boolean;
}

interface ChatMessage {
  id: string;
  content: string;
  direction: string;
  timestamp: number;
}

interface ConnectionInfo {
  state: string;
  peer_fingerprint: string | null;
  peer_verified: boolean;
  peer_key_hex: string | null;
}

interface FileRequest {
  peer_key_hex: string;
  transfer_id: string;
  filename: string;
  total_size: number;
}

function App() {
  const [view, setView] = useState<"setup" | "hub" | "chat">("setup");
  const [identity, setIdentity] = useState<IdentityInfo | null>(null);
  const [connection, setConnection] = useState<ConnectionInfo | null>(null);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [inputText, setInputText] = useState("");
  const [inviteToConnect, setInviteToConnect] = useState("");
  const [generatedInvite, setGeneratedInvite] = useState("");
  const [copied, setCopied] = useState(false);
  const [fileRequests, setFileRequests] = useState<FileRequest[]>([]);
  
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  // Initialize and check identity
  useEffect(() => {
    async function checkIdentity() {
      try {
        const info = await invoke<IdentityInfo>("init_identity");
        setIdentity(info);
        if (info.has_identity) {
          setView("hub");
        }
      } catch (err) {
        console.error("Init failed:", err);
      }
    }
    checkIdentity();
  }, []);

    const unlistenMsg = listen<any>("m2m://message", (event) => {
      setMessages((prev) => [...prev, event.payload.message]);
    });
    
    const unlistenConn = listen<any>("m2m://connection", async (event) => {
      const stateStr = event.payload.state;
      setConnection({
        state: stateStr,
        peer_fingerprint: event.payload.peer_fingerprint,
        peer_verified: false,
        peer_key_hex: event.payload.peer_key_hex,
      });
      if (stateStr === "established") {
        setView("chat");
        try {
          const history = await invoke<ChatMessage[]>("load_messages", { peerKeyHex: event.payload.peer_key_hex });
          setMessages(history);
        } catch (e) {
          console.error("Failed to load history", e);
        }
      } else if (stateStr === "disconnected") {
        setView("hub");
        setConnection(null);
        setMessages([]);
      }
    });

    const unlistenFileReq = listen<any>("m2m://file-request", (event) => {
      setFileRequests(prev => [...prev, event.payload]);
    });

    const unlistenFileComp = listen<any>("m2m://file-complete", (event) => {
      alert(`File transfer complete!\nSaved to: ${event.payload.path}`);
    });

    return () => {
      unlistenMsg.then(f => f());
      unlistenConn.then(f => f());
      unlistenFileReq.then(f => f());
      unlistenFileComp.then(f => f());
    };
  }, []);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  const handleGenerateInvite = async () => {
    try {
      await invoke("start_listening", { address: "127.0.0.1:0" });
      const address = await invoke<string>("get_listen_address");
      const invite = await invoke<string>("create_invite", {
        address,
        validityMinutes: 60,
        oneTime: true
      });
      setGeneratedInvite(invite);
    } catch (e) {
      console.error(e);
    }
  };

  const copyInvite = () => {
    navigator.clipboard.writeText(generatedInvite);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleConnect = async () => {
    if (!inviteToConnect) return;
    try {
      const info = await invoke<ConnectionInfo>("connect_to_peer", { inviteStr: inviteToConnect });
      setConnection(info);
      setView("chat");
    } catch (e) {
      console.error("Connection failed", e);
      alert("Connection failed: " + e);
    }
  };

  const handleSendMessage = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!inputText.trim() || !connection?.peer_key_hex) return;
    try {
      const msg = await invoke<ChatMessage>("send_message", {
        peerKeyHex: connection.peer_key_hex,
        content: inputText
      });
      setMessages((prev) => [...prev, msg]);
      setInputText("");
    } catch (e) {
      console.error(e);
    }
  };

  const handleVerify = async () => {
    if (!connection?.peer_key_hex) return;
    try {
      await invoke("verify_peer", { peerKeyHex: connection.peer_key_hex });
      setConnection({ ...connection, peer_verified: true });
    } catch(e) {
      console.error(e);
    }
  };

  const handleDisconnect = async () => {
    if (!connection?.peer_key_hex) return;
    try {
      await invoke("disconnect_peer", { peerKeyHex: connection.peer_key_hex });
      setView("hub");
      setConnection(null);
      setMessages([]);
    } catch (e) {
      console.error("Disconnect failed", e);
    }
  };

  const handleFileSelect = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file || !connection?.peer_key_hex) return;
    // Tauri doesn't easily expose full paths from standard web file inputs.
    // For a real app we'd use @tauri-apps/plugin-dialog, but for simplicity
    // we assume the user types the path, or we can just mock a UI here.
    // Since we need an absolute path for rust, let's use prompt for now.
    const path = prompt("Enter absolute path to file to send:");
    if (!path) return;
    
    try {
      await invoke("send_file", { peerKeyHex: connection.peer_key_hex, filePath: path });
      setMessages((prev) => [...prev, {
        id: Date.now().toString(),
        content: `Sent file request for: ${path}`,
        direction: "sent",
        timestamp: Math.floor(Date.now() / 1000)
      }]);
    } catch (err) {
      console.error(err);
      alert("Failed to send file: " + err);
    }
  };

  const acceptFile = async (req: FileRequest) => {
    const dir = prompt("Enter absolute directory path to save to:");
    if (!dir) return;
    try {
      await invoke("accept_file_transfer", { 
        peerKeyHex: req.peer_key_hex, 
        transferId: req.transfer_id,
        saveDir: dir
      });
      setFileRequests(prev => prev.filter(r => r.transfer_id !== req.transfer_id));
    } catch (err) {
      alert("Accept failed: " + err);
    }
  };

  const rejectFile = async (req: FileRequest) => {
    try {
      await invoke("reject_file_transfer", { 
        peerKeyHex: req.peer_key_hex, 
        transferId: req.transfer_id
      });
      setFileRequests(prev => prev.filter(r => r.transfer_id !== req.transfer_id));
    } catch (err) {
      alert("Reject failed: " + err);
    }
  };

  // Render Views
  if (view === "setup") {
    return (
      <div className="app-container">
        <div className="centered-view">
          <span style={{fontSize: '48px'}}>🔑</span>
          <h2>Initializing Secure Enclave...</h2>
          <p>Generating Ed25519 Identity keys. They never leave your device.</p>
        </div>
      </div>
    );
  }

  if (view === "hub") {
    return (
      <div className="app-container">
        <div className="header">
          <h1>🛡️ M2M Secure Messenger</h1>
          <div className="status-badge">Offline</div>
        </div>
        <div className="content-area centered-view">
          <div className="invite-section">
            <div className="card">
              <h3>➕ Host a Connection</h3>
              <p style={{fontSize: '0.85rem', marginTop: 0}}>Generate a one-time signature to allow a peer to connect to you.</p>
              {!generatedInvite ? (
                <button onClick={handleGenerateInvite}>Generate Invite Link</button>
              ) : (
                <div style={{display: 'flex', gap: '8px', alignItems: 'center'}}>
                  <input readOnly value={generatedInvite} />
                  <button onClick={copyInvite} style={{padding: '12px'}} title="Copy">
                    {copied ? "✔️" : "📋"}
                  </button>
                </div>
              )}
            </div>

            <div className="card">
              <h3>🔗 Join a Connection</h3>
              <p style={{fontSize: '0.85rem', marginTop: 0}}>Paste an invite link provided by a peer.</p>
              <div className="flex-row">
                <input 
                  placeholder="m2m://..." 
                  value={inviteToConnect}
                  onChange={(e) => setInviteToConnect(e.target.value)}
                />
                <button onClick={handleConnect}>Connect</button>
              </div>
            </div>
            
            <div className="fingerprint-box" style={{marginTop: '40px', textAlign: 'center'}}>
              <span style={{color: 'var(--text-muted)', display: 'block', marginBottom: '8px'}}>Your Fingerprint</span>
              {identity?.fingerprint}
            </div>
          </div>
        </div>
      </div>
    );
  }

  // Chat View
  return (
    <div className="app-container">
      <div className="header">
        <h1>
          {connection?.peer_verified ? (
            <span style={{marginRight: '8px'}}>✅</span> 
          ) : (
            <span onClick={handleVerify} style={{cursor:'pointer', marginRight: '8px'}} title="Click to verify fingerprint">⚠️</span>
          )}
          Encrypted Session
        </h1>
        <div style={{display: 'flex', gap: '10px', alignItems: 'center'}}>
          <div className={`status-badge ${connection?.state === 'established' ? 'connected' : 'disconnected'}`}>
            {connection?.state || 'Unknown'}
          </div>
          <button onClick={handleDisconnect} style={{padding: '6px 12px', fontSize: '0.8rem'}}>Disconnect</button>
        </div>
      </div>
      
      {fileRequests.length > 0 && (
        <div className="file-requests">
          {fileRequests.map(req => (
            <div key={req.transfer_id} className="file-request-banner">
              <span>Incoming File: {req.filename} ({Math.round(req.total_size/1024)} KB)</span>
              <div>
                <button onClick={() => acceptFile(req)} style={{marginRight: '8px', padding: '4px 8px'}}>Accept</button>
                <button onClick={() => rejectFile(req)} className="secondary" style={{padding: '4px 8px'}}>Reject</button>
              </div>
            </div>
          ))}
        </div>
      )}
      <div className="messages">
        <div style={{textAlign: 'center', opacity: 0.5, fontSize: '0.8rem', marginBottom: '20px'}}>
          Connected to peer.<br/>
          Fingerprint: <span style={{fontFamily: 'monospace'}}>{connection?.peer_fingerprint}</span>
        </div>
        {messages.map((m) => (
          <div key={m.id} className={`message-bubble ${m.direction}`}>
            {m.content}
            <span className="message-time">
              {new Date(m.timestamp * 1000).toLocaleTimeString([], {hour: '2-digit', minute:'2-digit'})}
            </span>
          </div>
        ))}
        <div ref={messagesEndRef} />
      </div>
      <form className="input-area" onSubmit={handleSendMessage}>
        <button type="button" onClick={() => {
            const path = prompt("Enter absolute path to file to send:");
            if (path) {
                invoke("send_file", { peerKeyHex: connection?.peer_key_hex, filePath: path })
                .catch(e => alert("Failed: " + e));
            }
        }} style={{padding: '12px', background: 'transparent', border: '1px solid var(--border)'}} title="Send File">
          📎
        </button>
        <input 
          placeholder="Type a secure message..." 
          value={inputText}
          onChange={(e) => setInputText(e.target.value)}
        />
        <button type="submit" style={{padding: '12px', display: 'flex', alignItems: 'center', justifyContent: 'center'}}>
          <span>🚀</span>
        </button>
      </form>
    </div>
  );
}

export default App;
