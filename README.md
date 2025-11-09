# Parlance

A decentralized P2P messaging application.

## Demo
![Parlance Demo](demo/parlance-demo.gif)

## Overview
Parlance is a functional P2P messaging application that works both on local networks and across the internet. It uses UDP multicast for local peer discovery and a lightweight bootstrap server for internet-scale discovery. All messaging happens directly peer-to-peer.

## Current State

**Working:**
- **Local discovery**: UDP multicast for automatic LAN peer discovery
- **Internet discovery**: WebSocket-based bootstrap server for cross-network discovery
- **Mode selection**: Choose local or internet discovery
- Direct TCP messaging between discovered peers
- Multiple instances on the same machine (SO_REUSEPORT)

**Limitations:**
- No NAT traversal yet (requires direct connectivity or port forwarding)
- No encryption (cleartext over TCP)
- No message persistence
- No group chat (only 1-to-1 messaging)

## Architecture

**Local Discovery (UDP Multicast):**
- Multicast group: `239.255.255.250:6789`
- JSON-serialized discovery messages

**Internet Discovery (Bootstrap Server):**
- WebSocket-based signaling server
- Maintains registry of online peers

**Messaging Layer (TCP):**
- Each peer listens on a dynamically assigned port
- Direct socket connections for message delivery
- Line-delimited JSON over TCP
- Concurrent connection handling via Tokio

## Building

Requires Rust 1.70 or later.

```bash
# Build everything
cargo build --workspace
```

## Usage

### Local Network Mode (Default)

Start multiple instances on the same network:

```bash
# Terminal 1
cargo run -p parlance -- --nickname alice

# Terminal 2
cargo run -p parlance -- --nickname bob
```

Discovery happens automatically via UDP multicast. After a few seconds, peers will appear in each other's peer lists.

### Internet Mode

**Step 1:** Start the bootstrap server:

```bash
# Terminal 1
cargo run -p bootstrap-server -- --host 0.0.0.0 --port 8080
```

**Step 2:** Start clients with `--mode internet`:

```bash
# Terminal 2
cargo run -p parlance -- --nickname alice --mode internet

# Terminal 3 (can be on a different network!)
cargo run -p parlance -- --nickname bob --mode internet
```

The clients will automatically connect to the bootstrap server and discover each other.

### Configuration

Edit `parlance-client/parlance.toml` to configure discovery mode:

```toml
[network]
mode = "local"  # Options: local | internet
bootstrap_server = "ws://localhost:8080"
```

- `local`: Use UDP multicast (LAN only, default)
- `internet`: Use bootstrap server (cross-network)

**Commands:**
- `/peers` - Show discovered peers
- `/send <nickname> <message>` - Send a message
- `/quit` - Exit
- `/help` - Show help

**Example:**
```
/peers
/send bob hey, testing this out
```

Messages appear in the recipient's terminal with a timestamp.

## Protocol Details

### Discovery Protocol

Peers send periodic announcements to `239.255.255.250:6789`:

```json
{
  "type": "announce",
  "nickname": "alice",
  "tcp_port": 54321
}
```

The peer registry maintains a list of all recently-seen peers. Peers are removed if they haven't announced in 15 seconds.

### Messaging Protocol

Messages are sent over TCP as line-delimited JSON:

```json
{
  "from": "alice",
  "content": "message text",
  "timestamp": 1699123456
}
```

Each peer maintains a TCP listener. To send a message, a peer:
1. Looks up the recipient in the peer registry
2. Opens a TCP connection to their address
3. Sends the JSON message followed by `\n`
4. Closes the connection

This is inefficient but simple.