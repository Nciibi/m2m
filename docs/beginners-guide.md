# M2M Secure Messenger: A Beginner's Guide

Welcome to the **M2M (Machine-to-Machine) Secure Messenger** project! If you are new to computer science, cybersecurity, or software engineering, this document is designed specifically for you. 

We will break down exactly how this application works, piece by piece, explaining the "why" and "how" behind every major decision. By the end of this guide, you will understand how modern, secure, peer-to-peer applications are built.

---

## 1. What is M2M?

Most messaging apps (like WhatsApp, Discord, or Telegram) use a **client-server model**. When you send a message, it goes from your phone to a central server owned by a company, and then that server forwards it to your friend. 

**M2M is different. It is a Peer-to-Peer (P2P) application.** 
There are no central servers. When you send a message to a friend, your computer connects *directly* to their computer. Because of this, it is impossible for a third-party company to read your messages, shut down your servers, or harvest your data.

To achieve this, M2M uses highly advanced **End-to-End Encryption (E2EE)** to ensure that even if someone intercepts the network traffic, all they see is random mathematical noise.

---

## 2. High-Level Architecture

Building a desktop application requires combining different technologies. M2M uses a framework called **Tauri**. 

Tauri allows us to build the visual User Interface (UI) using web technologies (HTML, CSS, React) while doing all the heavy lifting (networking, cryptography, file saving) in a blazing-fast, secure systems programming language called **Rust**.

### The Architecture Diagram

```mermaid
graph TD
    subgraph Your Computer
        UI[Frontend: React + TypeScript]
        Backend[Backend: Rust Core]
        DB[(Encrypted Local SQLite DB)]
    end
    
    subgraph Friend's Computer
        F_Backend[Backend: Rust Core]
    end

    UI <==>|Tauri IPC Commands| Backend
    Backend <==>|Read / Write| DB
    Backend <==>|Encrypted TCP Socket| F_Backend
```

**How they talk to each other:**
1. **Frontend (React)**: This is what you see. The buttons, the chat bubbles, and the inputs. It handles the logic of displaying data to the user.
2. **IPC (Inter-Process Communication)**: When you click "Send", the React frontend sends a message to the Rust backend using an IPC command. Think of this as a secure bridge between the web interface and the operating system.
3. **Backend (Rust)**: Rust receives the message, encrypts it, and pushes it out to the internet over a **TCP Socket** directly to your friend.

---

## 3. Cryptography Explained (For Beginners)

Cryptography sounds intimidating, but it revolves around a few core concepts. M2M uses a famous and highly audited library called **libsodium** to handle this safely.

### Public and Private Keys (Asymmetric Cryptography)
When you first open M2M, it generates an **Identity Keypair** using an algorithm called `Ed25519`.
*   **Private Key**: A secret password only your computer knows. It never leaves your device.
*   **Public Key**: A mathematically related identifier that you share with the world. 

If someone wants to verify a message came from you, they use your Public Key. If you want to prove you sent it, you stamp it with your Private Key (a "Digital Signature").

### The Handshake (Key Exchange)
Before you and your friend can chat, your computers must agree on a shared secret password to lock the messages. But how do you agree on a secret over the internet without someone listening in? 

We use an algorithm called `X25519`. You combine your Private Key with your friend's Public Key, and magically, both computers arrive at the exact same mathematical number. This number becomes the **Session Key**.

```mermaid
sequenceDiagram
    participant Alice
    participant Bob

    Note over Alice,Bob: 1. Connection Established
    Alice->>Bob: Hello! Here is my Public Key & Signature
    Bob->>Alice: Hello! Here is my Public Key & Signature
    Note over Alice,Bob: 2. Mathematical Magic (X25519)
    Note over Alice: Calculates Session Key
    Note over Bob: Calculates Session Key
    Note over Alice,Bob: 3. Secure Channel Open!
    Alice->>Bob: [Encrypted Message]
```

### Locking the Messages (Symmetric Encryption)
Now that both computers have the **Session Key**, they use an algorithm called `XChaCha20-Poly1305` to encrypt the actual text messages. Because both computers have the same key, it is extremely fast to lock (encrypt) and unlock (decrypt) the messages.

---

## 4. How the Network Works

To connect directly to another computer, you need an **IP Address** (like a house address) and a **Port** (like a specific door on that house). 

When you click "Host a Connection", your computer opens a specific door (port) and waits. M2M generates an **Invite String** (which is just your IP, Port, and Public Key bundled together). You give this string to your friend.

### Framing the Data
When computers talk over TCP, they just send a continuous stream of 1s and 0s. The receiving computer needs to know where one message ends and the next begins. 

To solve this, M2M uses **Length-Prefixed Framing**. Before sending a message, it attaches a tiny 4-byte header that says, "The following message is exactly 142 bytes long." The receiving computer reads the 4 bytes, knows exactly how much data to wait for, and then cuts the stream perfectly.

---

## 5. Secure File Transfers

Sending a tiny text message is easy. Sending a massive 2-Gigabyte video file is hard. You cannot load a 2GB file into memory all at once without crashing the app. 

M2M solves this using **Chunking**.

```mermaid
graph LR
    File[Large Video File] --> Chunk1[Chunk 1: 1MB]
    File --> Chunk2[Chunk 2: 1MB]
    File --> Chunk3[Chunk 3: 1MB]
    
    Chunk1 --> Encrypt[Encrypt]
    Chunk2 --> Encrypt
    Chunk3 --> Encrypt
    
    Encrypt --> Network((Internet))
```

1. **Request**: Your computer asks the peer, "Can I send a file named `video.mp4`? It is 2GB."
2. **Chunking**: If they accept, your computer reads the file 1 Megabyte at a time. 
3. **Hashing**: For every chunk, it calculates a mathematical fingerprint called a **Hash** (using `SHA256`). This ensures that if the internet connection glitches, the receiving computer knows the chunk is corrupted and drops it.
4. **Reassembly**: The receiving computer gets the chunks, decrypts them, verifies the hashes, and writes them directly to the hard drive, one by one.

---

## 6. Local Storage

What happens to your messages when you close the app? In M2M, they are saved locally on your hard drive using a database called **SQLite**. 

However, we don't want anyone who steals your laptop to be able to read your chat history. Therefore, before a message is saved to the SQLite database, it is encrypted using a **Storage Key**. 

This ensures that the database file sitting on your hard drive looks entirely like random noise to anyone snooping around your computer files. When you open the app, it loads the database, decrypts the messages in real-time, and displays them on your screen.

---

## 7. Conclusion

By combining React for a beautiful User Interface, Rust for high-performance memory safety, and libsodium for military-grade cryptography, M2M creates a messaging environment that is fast, resilient, and entirely private. 

---

## 8. Codebase Deep Dive: How the Pieces Fit Together

Now that you understand the high-level concepts, let's look at the actual code! A project like this is split into many files so that it stays organized. Here is a guided tour of the M2M codebase and how data flows from a button click in the UI all the way to a secure network socket.

### Step 1: The Frontend (`src/App.tsx`)
This is where the user interface lives. We use **React**, a JavaScript library for building UI components. 

When you type a message and hit "Send", React doesn't know how to encrypt data or open network ports. Instead, it asks the Rust backend to do it using Tauri's `invoke` command:

```typescript
// From App.tsx
const handleSendMessage = async (e: React.FormEvent) => {
  e.preventDefault();
  // Call the Rust function named "send_message"
  const msg = await invoke<ChatMessage>("send_message", {
    peerKeyHex: connection.peer_key_hex,
    content: inputText
  });
  // Update the UI with the sent message
  setMessages((prev) => [...prev, msg]);
};
```
React also listens for events *from* Rust (like when your friend sends you a message) using the `listen` function:
```typescript
const unlistenMsg = listen<any>("m2m://message", (event) => {
  // When a message arrives from Rust, put it on the screen!
  setMessages((prev) => [...prev, event.payload.message]);
});
```

### Step 2: The Command Bridge (`src-tauri/src/commands.rs`)
When React calls `invoke("send_message")`, Tauri looks for a Rust function marked with the `#[tauri::command]` tag. `commands.rs` acts as the **bridge** between JavaScript and Rust.

```rust
// From commands.rs
#[tauri::command]
pub async fn send_message(
    state: State<'_, Arc<AppState>>,
    peer_key_hex: String,
    content: String,
) -> Result<ChatMessage, String> {
    // 1. Look up the active connection for this peer
    let conns = state.connections.read().await;
    let conn_arc = conns.get(&peer_key_hex).unwrap();
    let mut conn = conn_arc.lock().await;

    // 2. Ask the session to encrypt and send the message over the network
    conn.session.send_text(&mut conn.write_half, &content).await;

    // 3. Save the encrypted message to the local SQLite database
    state.message_store.lock().await.store_message(...);

    // 4. Return success to React!
    Ok(ChatMessage { ... })
}
```

### Step 3: Managing the State (`src-tauri/src/state.rs`)
Notice the `state` variable in the function above? In a web app, variables are usually isolated to one user's request. But a desktop app stays open for hours, managing multiple background connections simultaneously. 

`state.rs` defines the `AppState` struct—a giant container that holds the app's memory:
*   `connections`: A dictionary (HashMap) of all active peer connections.
*   `message_store`: The SQLite database connection for saving chat history.
*   `identity`: Your secret keys.

Because multiple tasks (like the UI clicking a button, and the network receiving a packet) might try to access the `AppState` at the *exact same time*, we wrap everything in a **Mutex** (Mutual Exclusion lock). A Mutex forces tasks to form an orderly line, preventing memory corruption.

### Step 4: The Session State Machine (`src-tauri/src/session.rs`)
When `commands.rs` calls `session.send_text()`, it hands the message to the **Session**. 

The Session represents the mathematical and cryptographic state of a connection. It holds the shared `Session Key` we talked about in the Cryptography section. Its job is to take raw text, encrypt it into cipher-bytes using `libsodium`, and format it into a `PacketType::EncryptedMessage`.

```rust
// Inside session.rs
pub fn encrypt_message(&mut self, body: MessageBody) -> Result<Vec<u8>, ProtocolError> {
    // Use the shared symmetric key (XChaCha20-Poly1305) to lock the message
    let ciphertext = aead::seal(&plaintext, None, &nonce, &self.tx_key);
    Ok(ciphertext)
}
```

### Step 5: The Network Layer (`src-tauri/src/network.rs`)
Once the Session has encrypted the data, it hands the raw bytes down to the lowest level: the Network layer.

The Network layer doesn't care about encryption or chat messages. Its only job is to push bytes out to the physical internet via **TCP Sockets**. It also implements the **Length-Prefixed Framing** we discussed earlier. 

```rust
// Inside network.rs
pub async fn write_frame<W: AsyncWrite + Unpin>(stream: &mut W, frame: RawFrame) {
    // 1. Calculate how big the frame is
    let length = frame.payload.len() as u32;
    // 2. Send the 4-byte size header first
    stream.write_u32(length).await;
    // 3. Send the actual payload
    stream.write_all(&frame.payload).await;
}
```

### Summary of the Data Flow
Let's trace a message from start to finish:
1. You type "Hello" and click Send in **`App.tsx`**.
2. React triggers an IPC command which calls `send_message` in **`commands.rs`**.
3. `commands.rs` grabs the active connection from **`state.rs`**.
4. The raw text is passed to **`session.rs`**, which uses `libsodium` to encrypt "Hello" into unreadable cipher-bytes.
5. The cipher-bytes are passed to **`network.rs`**, which calculates the size, prepends a 4-byte header, and fires it over the TCP socket.
6. `commands.rs` then uses **`storage.rs`** to securely encrypt the message *again* and save it to your local SQLite database for later.
7. Finally, the function returns, and React displays your chat bubble!

This layered architecture (UI -> Commands -> Session -> Network) ensures that the code stays clean. The Network layer never has to worry about UI buttons, and the UI never has to worry about TCP byte headers!
