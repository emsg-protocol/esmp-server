# ESMP Server

## Overview

**ESMP (Encrypted Secure Messaging Protocol)** is a decentralized messaging protocol designed to replace traditional email concepts with modern, secure, and group-oriented messaging. The ESMP server provides a reference implementation for the protocol, focusing on cryptographic security, group messaging, and extensibility.

## Features
- **Decentralized, JSON-based messaging**
- **Cryptographic signing and verification** (Ed25519)
- **Group chat support** with persistent threads (`group_id`)
- **System messages** for group events (`joined`, `left`, `removed`, `group_created`)
- **Direct and group messaging**
- **Runs on TCP port 5888** (ESMP protocol)

## Port 5888 Usage
The ESMP server listens for incoming TCP connections on port **5888**. Each connection can send newline-delimited JSON messages. Every message must be cryptographically signed by the sender.

## JSON Message Format
All messages must be valid JSON and include the following fields:

```
{
  "to": ["user1#domain.com", "user2#domain.com"],
  "cc": ["user3#domain.com"],           // Optional
  "group_id": "group-uuid",            // Optional, for group chat
  "type": "text",                      // Message type or system event
  "body": { ... },                       // Message content (arbitrary JSON)
  "signature": "base64-ed25519-sig",   // Ed25519 signature (base64)
  "sender_pubkey": "base64-pubkey"     // Sender's Ed25519 public key (base64)
}
```

### System Message Types
For group management, the `type` field can be one of:
- `joined`
- `left`
- `removed`
- `group_created`

These are used to signal group membership and administrative events.

## Security
- **All messages must be signed** with Ed25519. Unsigned or tampered messages are rejected.
- The server verifies the signature using the provided `sender_pubkey` and the canonical JSON of the message (excluding `signature` and `sender_pubkey`).

## Group Chat
- Messages with a `group_id` are treated as group chat and persisted under a unique thread for that group.
- System messages are logged and can be used to manage group state.

## Running the Server
The server is written in Rust and uses async networking. To run:

```
cargo run --release
```

The server will listen on TCP port 5888 for ESMP protocol messages.

---

For more details, see the `src/esmp/` directory for protocol logic and implementation.
