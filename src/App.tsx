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

function App() {
  const [view, setView] = useState<"setup" | "hub" | "chat">("setup");
  const [identity, setIdentity] = useState<IdentityInfo | null>(null);
  const [connection, setConnection] = useState<ConnectionInfo | null>(null);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [inputText, setInputText] = useState("");
  const [inviteToConnect, setInviteToConnect] = useState("");
  const [generatedInvite, setGeneratedInvite] = useState("");
  const [copied, setCopied] = useState(false);
  
  const messagesEndRef = useRef<HTMLDivElement>(null);

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

  // Listen for Tauri events
  useEffect(() => {
    const unlistenMsg = listen<any>("m2m://message", (event) => {
      setMessages((prev) => [...prev, event.payload.message]);
    });
    
    const unlistenConn = listen<any>("m2m://connection", (event) => {
      const stateStr = event.payload.state;
      setConnection({
        state: stateStr,
        peer_fingerprint: event.payload.peer_fingerprint,
        peer_verified: false,
        peer_key_hex: event.payload.peer_key_hex,
      });
      if (stateStr === "established") {
        setView("chat");
      } else if (stateStr === "disconnected") {
        setView("hub");
        setConnection(null);
      }
    });

    return () => {
      unlistenMsg.then(f => f());
      unlistenConn.then(f => f());
    };
  }, []);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  const handleGenerateInvite = async () => {
    try {
      // For MVP, we listen on a random port or predefined localhost address
      // Make sure the listener is started
      await invoke("start_listening", { address: "127.0.0.1:0" });
      const invite = await invoke<string>("create_invite", {
        address: "127.0.0.1:8080", // Hardcoded MVP address hint for local testing
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
        <div className={`status-badge ${connection?.state === 'established' ? 'connected' : 'disconnected'}`}>
          {connection?.state || 'Unknown'}
        </div>
      </div>
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
