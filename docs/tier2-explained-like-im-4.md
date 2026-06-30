# M2M Tier 2 — Explained Like I'm 4 🧸

---

## Before: How M2M Used To Work

Imagine you and your best friend each have **a secret walkie-talkie**.

**To talk to each other, you had to:**
1. 🏠 Take your walkie-talkie out of your pocket
2. ✉️ Write a special **invitation letter** with your address on it
3. 📮 Give that letter to your friend — through Signal, WhatsApp, or by reading it out loud
4. 🔗 Your friend types the letter into their walkie-talkie
5. 📞 Now you can talk!

**The problem?**
- If your friend loses the letter, they can't call you 😢
- If you move to a different house (change IP addresses), the letter is wrong
- If your WiFi blinks off, you have to write a NEW letter
- If you're on the same WiFi, you STILL need a letter (silly, right?)

---

## After: What M2M Can Do Now

Now your walkie-talkie got **three superpowers** 🦸‍♀️

---

## Superpower #1: LAN Discovery — "The Magic Neighbor Finder" 🏠✨

**What it does:**
Your M2M app now shouts "I'M HERE! 🙋‍♀️" every 30 seconds on your home WiFi.
Any other M2M app on the same WiFi hears it and says "HI! 👋"

**How it works (the easy version):**
```
Your computer:   "HELLO! I'm M2M at 192.168.1.5, port 9876!"
Friend's laptop: "Oh hi! I'll add you to my friend list!" 
                 🤝 Automatically connected!
```

**The grown-up details:**
- Your app sends a tiny **signed shout** over the network every 30 seconds
- The shout is signed with your secret key — nobody can fake it
- If a friend's app doesn't hear from you for 90 seconds, it assumes you went to sleep 😴
- **Safe**: The shout only contains your public fingerprint (like your name), not your messages

**Before → After:**
| Before | After |
|--------|-------|
| Need a special invite link to connect | Open M2M on the same WiFi → they find each other |
| Can't discover nearby friends | Friends on the same network appear in your list automatically |
| Only internet connections | Zero-config local connections too |

---

## Superpower #2: DHT — "The Phone Book for the Internet" 📖🌐

**What it does:**
Imagine a magical **phone book** that every M2M user shares. When you go online, you write your address in it. When your friend looks for you, they find you instantly.

**How it works (the easy version):**
```
You go online → You write your name + address in the shared phone book 📝
Your friend looks for you → Finds your address → Calls you directly 📞
No invite letter needed! 🎉
```

**The grown-up details:**
- DHT stands for "Distributed Hash Table" — fancy words for "phone book that lives on many computers"
- Your app talks to **bootstrap nodes** (a few friendly computers that help start the phone book)
- You periodically announce: "I'm here! My peer ID is XYZ, my IP is 1.2.3.4:9876"
- When someone searches for your peer ID, the DHT finds you in **O(log N) steps** — very fast!
- **Privacy**: You can turn this OFF in Settings. When off, you're invisible to DHT and must use invite links (exactly like before).

**Before → After:**
| Before | After |
|--------|-------|
| Must share an invite link every single time | Share your peer ID **once** — friends can always find you |
| Friend needs the invite to connect | Friend just types your peer ID → DHT finds you |
| If your IP changes, old invites don't work | DHT always has your current address |
| Public keys are only in invite links | Public keys are in the DHT phone book |

**When you'd still use invite links:**
- First contact (to exchange peer IDs securely)
- Super private mode (DHT turned OFF)
- One-time secret conversations

---

## Superpower #3: Reconnection — "The Don't-Give-Up" 💪🔄

**What it does:**
If your WiFi blinks off, M2M doesn't give up. It says "I'll wait and try again!" like a patient puppy 🐕

**How it works (the easy version):**
```
WiFi drops:    "Oh no! Where'd my friend go?" 😟
Wait 1 sec:    "Are you back yet?"
Wait 2 secs:   "How about now?"
Wait 4 secs:   "Hello? Helloooo?"
Wait 8 secs:   "...anyone there?"
Wait 16 secs:  "I'll try one more time..."
Wait 30 secs:  "Okay, I give up for now. Tap Retry to try again!"
                ⬆️ Each wait is longer — like counting before hide-and-seek
```

**The grown-up details:**
- Uses **exponential backoff**: 1s → 2s → 4s → 8s → 16s → 30s cap
- After 5 failures, it stops and says "I give up" (you can tap Retry)
- On successful reconnect, it does a **fresh X3DH handshake** — new secret keys each time!
- **Messages you wrote while offline** are saved and sent when reconnected
- **File transfers** resume from where they stopped (not from the beginning!)

**Before → After:**
| Before | After |
|--------|-------|
| WiFi blips → conversation dies | WiFi blips → auto-reconnects in seconds |
| Must create new invite to talk again | Connection resumes automatically |
| Message lost if friend is offline | Message saved, sent when friend comes back |
| File transfer at 80% → failed → restart from 0% | File transfer resumes from last received chunk |
| Nothing happens visually | Shows "Reconnecting…" badge so you know it's trying |

---

## Summary: The 3 New Files in Your App 📁

| File | What is it? | Size |
|------|-------------|:----:|
| `src-tauri/src/lan_discovery.rs` | 🏠 **LAN Finder** — shouts on WiFi every 30s | ~240 lines |
| `src-tauri/src/dht.rs` | 📖 **Internet Phone Book** — find friends online | ~420 lines |
| `src-tauri/src/reconnect.rs` | 💪 **Don't-Give-Up Engine** — auto-reconnect | ~75 lines |

Plus we **added a pocket** to your app's backpack (state.rs) to remember reconnection info, and **taught the message storage** (storage.rs) how to save messages you wrote while offline.

---

## What This Means For Your App

### The Old Way (before these changes)
```
Generate invite → Share link → Friend pastes link → Talk
                ↓ WiFi drops → Start over
```

### The New Way (with these changes)
```
Install M2M on two devices on same WiFi → They find each other ✨
                    OR
Share your peer ID once → Friend types it → DHT finds you → Talk forever
                    ↓
WiFi drops → Auto-reconnects in 1-30 seconds → Keep talking 💪
                    ↓
Friend goes offline → Your messages saved → Sent when they're back 📬
```

### Settings You'll See (when wired to the UI)
```
Settings → Discovery:
  ☐ LAN Discovery [ON/OFF] — Find friends on same WiFi
  ☐ DHT Peer Discovery [ON/OFF] — Be findable on the internet
  
When DHT is OFF → Works exactly like before (invite links only)
When DHT is ON  → Friends who know your peer ID can find you
When LAN is ON  → Friends on same WiFi appear automatically
```

---

## Did We Remove Invite Links?

**NO!** Invite links are still here. They're the **most secure** way to connect. The new stuff is *extra* — like adding a phone book and a neighbor-finder to your walkie-talkie, but you can still write invite letters whenever you want.

Think of it like:
- **Invite link** = giving your friend a key to your house 🔑
- **DHT** = being in the phone book 📖
- **LAN** = waving from the window next door 🪟

You can use one, two, or all three. Your choice!

---

## Tests We Added (to make sure it works!)

| Test | What it checks |
|------|----------------|
| 🧪 `build_announcement_success` | The LAN shout packet is the right size |
| 🧪 `parse_valid_announcement` | A friend's shout can be understood |
| 🧪 `parse_rejects_bad_signature` | A fake shout gets ignored |
| 🧪 `expire_stale_peers` | A friend who left gets removed from the list |
| 🧪 `build_dht_message` | The DHT message format is correct |
| 🧪 `parse_node_response` | A DHT search result can be read |
| 🧪 `compute_backoff_exponential` | The retry waits: 1s, 2s, 4s... |
| 🧪 `compute_backoff_capped` | The retry never waits more than 30s |

**Total: 20 new tests, all passing ✅**

**All 210 old tests still pass ✅**
**Total: 230 tests, 0 failed** 🏆
