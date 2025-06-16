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

```json
{
  "to": ["user1#domain.com", "user2#domain.com"],
  "cc": ["user3#domain.com"],           // Optional
  "group_id": "group-uuid",             // Optional, for group chat
  "type": "text | system",              // Message type
  "body": { ... },                      // Message content (arbitrary JSON)
  "signature": "base64-ed25519-sig",    // Ed25519 signature (base64)
  "sender_pubkey": "base64-pubkey",     // Sender's Ed25519 public key (base64)

  // The following fields are required for system messages:
  "subtype": "...",                     // One of the system message types below
  "actor": "user#domain.com",           // Who performed the action
  "target": "user#domain.com",          // Required for some system messages
  "timestamp": "2025-06-16T10:00:00Z",  // RFC3339 timestamp

  // Optional metadata for specific system messages:
  "new_name": "string",                 // For group_renamed
  "new_description": "string",          // For description_updated  
  "new_dp_url": "string"               // For dp_updated
}
```

### System Message Types
System messages (`type: "system"`) use the following subtypes:

Group Membership:
- `joined` - User joined the group
- `left` - User left voluntarily  
- `removed` - User was removed by admin
- `admin_assigned` - User was made admin
- `admin_revoked` - Admin privileges revoked

Group Settings:
- `group_created` - New group created
- `group_renamed` - Group name changed
- `description_updated` - Group description changed
- `dp_updated` - Display picture updated

User Profile:
- `profile_updated` - User profile fields updated (changes field indicates which fields)

### Group Metadata
Each group maintains metadata that is updated by system messages:

```json
{
  "group_id": "group-uuid",
  "group_name": "string",
  "group_description": "string", 
  "group_dp_url": "string",
  "admins": ["user1#domain.com"],
  "members": ["user1#domain.com", "user2#domain.com"],
  "created_at": "2025-06-16T10:00:00Z",
  "updated_at": "2025-06-16T10:00:00Z"
}
```

### User Profiles
Each user can maintain an optional profile with personal information:

```json
{
  "pubkey": "base64-ed25519-pubkey",
  "first_name": {
    "value": "string",           // Optional
    "visibility": "public|private"  // Default: private
  },
  "middle_name": {
    "value": "string",           // Optional
    "visibility": "public|private"  // Default: private
  },
  "last_name": {
    "value": "string",           // Optional
    "visibility": "public|private"  // Default: private
  },
  "display_picture": {
    "value": "string",           // Optional, URL to profile picture
    "visibility": "public|private"  // Default: private
  },
  "address": {
    "value": "string",           // Optional, always private & encrypted
    "visibility": "private"      // Always private
  },
  "updated_at": "2025-06-16T10:00:00Z"
}
```

Profile fields have the following validation and privacy rules:

Validation:
- Names (first, middle, last):
  - Maximum 50 characters
  - Only letters, spaces, hyphens and apostrophes allowed
- Display picture:  
  - Must be a valid URL
- Address:
  - Maximum 200 characters
  - Always stored encrypted at rest

Privacy:
- Each field (except address) can be marked as public or private
- Private fields are only visible to the profile owner
- Address is always private and encrypted in storage
- Non-owners only see public fields

Profile updates must be signed by the user's private key. The server exposes the following HTTP endpoints:

- `GET /users/{pubkey}/profile` - Get a user's profile
- `PUT /users/{pubkey}/profile` - Update a user's profile (requires signature)

The PUT endpoint returns HTTP 400 for validation errors with a descriptive error message.

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
