# Lobster Dashboard WebSocket Protocol v1.0.0

This document specifies the WebSocket streaming protocol used to communicate
between the Lobster Dashboard server (Python) and visualization clients
(bisque-computer, or any other consumer).

## Connection

- **Transport:** WebSocket (RFC 6455)
- **Default endpoint:** `ws://<host>:9100`
- **Subprotocol:** None (plain WebSocket)
- **Encoding:** UTF-8 JSON text frames

## Message Frame Format

Every message (server-to-client and client-to-server) is a JSON object with
the following top-level fields:

```json
{
  "version": "1.0.0",
  "type": "<message_type>",
  "timestamp": "2026-02-21T09:30:00.000000+00:00",
  "data": { ... }
}
```

| Field       | Type   | Required | Description                                    |
|-------------|--------|----------|------------------------------------------------|
| `version`   | string | yes      | Protocol version (semver)                      |
| `type`      | string | yes      | Message type identifier                        |
| `timestamp` | string | yes      | ISO 8601 timestamp with timezone               |
| `data`      | object | no       | Payload (absent for `ping`/`pong`)             |

## Server-to-Client Message Types

### `hello`

Sent immediately upon connection. Contains server metadata.

```json
{
  "version": "1.0.0",
  "type": "hello",
  "timestamp": "...",
  "data": {
    "server": "lobster-dashboard",
    "protocol_version": "1.0.0"
  }
}
```

### `snapshot`

Full state dump. Sent once after `hello`, and on-demand when the client
sends `request_snapshot`. Contains the complete Lobster instance state.

```json
{
  "version": "1.0.0",
  "type": "snapshot",
  "timestamp": "...",
  "data": {
    "system": { ... },
    "sessions": [ ... ],
    "message_queues": { ... },
    "tasks": { ... },
    "scheduled_jobs": [ ... ],
    "task_outputs": [ ... ],
    "recent_memory": [ ... ],
    "conversation_activity": { ... },
    "filesystem": [ ... ],
    "health": { ... }
  }
}
```

### `update`

Periodic state update. Same schema as `snapshot.data`. Sent at the server's
configured interval (default: every 3 seconds). In future versions, this
may contain only changed fields (delta updates).

### `pong`

Response to a client `ping`.

```json
{
  "version": "1.0.0",
  "type": "pong",
  "timestamp": "..."
}
```

### `error`

Server-side error notification.

```json
{
  "version": "1.0.0",
  "type": "error",
  "timestamp": "...",
  "data": {
    "message": "Description of the error"
  }
}
```

## Client-to-Server Message Types

### `ping`

Keepalive / latency check. Server responds with `pong`.

```json
{ "type": "ping" }
```

### `request_snapshot`

Request an immediate full state dump.

```json
{ "type": "request_snapshot" }
```

## Data Schemas

### `system`

Host-level system information.

```json
{
  "hostname": "ip-172-31-27-127",
  "platform": "Linux",
  "platform_version": "#1 SMP ...",
  "architecture": "x86_64",
  "python_version": "3.13.5",
  "boot_time": "2026-01-23T22:21:19+00:00",
  "uptime_seconds": 2459307,
  "cpu": {
    "count": 8,
    "percent": 12.5,
    "load_avg": [0.19, 0.44, 0.57]
  },
  "memory": {
    "total_mb": 32101,
    "used_mb": 15632,
    "available_mb": 16469,
    "percent": 48.7
  },
  "disk": {
    "total_gb": 98.2,
    "used_gb": 68.6,
    "free_gb": 25.5,
    "percent": 72.9
  }
}
```

### `sessions`

Array of detected Claude Code processes.

```json
[
  {
    "pid": 12345,
    "name": "claude",
    "cmdline": "claude --dangerously-skip-permissions",
    "started": "2026-02-18T11:26:54.300000+00:00",
    "cpu_percent": 0.5,
    "memory_mb": 699.7
  }
]
```

### `message_queues`

Counts and recent messages for each queue directory.

```json
{
  "inbox": {
    "count": 0,
    "recent": [
      {
        "id": "1771666140031_self",
        "source": "system",
        "chat_id": 0,
        "text": "status? (Self-check)",
        "timestamp": "2026-02-21T09:29:00.030059"
      }
    ]
  },
  "processing": { "count": 0 },
  "processed": { "count": 5827 },
  "sent": { "count": 341 },
  "outbox": { "count": 0 },
  "failed": { "count": 2 },
  "dead_letter": { "count": 2 }
}
```

### `tasks`

Current task list from tasks.json.

```json
{
  "tasks": [
    {
      "id": 1,
      "subject": "Test scheduled job created",
      "description": "Testing the scheduled tasks system",
      "status": "completed",
      "created_at": "2026-01-24T09:29:13+00:00",
      "updated_at": "2026-02-08T00:06:06+00:00"
    }
  ],
  "next_id": 3,
  "summary": {
    "total": 1,
    "pending": 0,
    "in_progress": 0,
    "completed": 1
  }
}
```

### `scheduled_jobs`

Array of scheduled job definitions.

```json
[
  {
    "name": "nightly-github-backup",
    "file": "/home/admin/lobster/scheduled-tasks/tasks/nightly-github-backup.md",
    "size_bytes": 2048,
    "modified": "2026-01-30T01:36:00+00:00"
  }
]
```

### `task_outputs`

Array of recent task output records (newest first).

```json
[
  {
    "job_name": "nightly-github-backup",
    "timestamp": "2026-02-20T00:00:15+00:00",
    "status": "success",
    "output": "Backed up 5 repos..."
  }
]
```

### `recent_memory`

Array of recent memory events from the vector store.

```json
[
  {
    "id": 42,
    "timestamp": "2026-02-21T08:00:00+00:00",
    "type": "message",
    "source": "telegram",
    "project": null,
    "content": "Truncated content (max 300 chars)...",
    "consolidated": false
  }
]
```

### `conversation_activity`

Aggregate conversation metrics.

```json
{
  "messages_received_1h": 3,
  "messages_received_24h": 47,
  "replies_sent_1h": 2,
  "replies_sent_24h": 38,
  "failed_1h": 0,
  "failed_24h": 0
}
```

### `filesystem`

Overview of key Lobster directories.

```json
[
  {
    "path": "messages/inbox",
    "absolute_path": "/home/admin/messages/inbox",
    "file_count": 0,
    "exists": true
  }
]
```

### `health`

Lobster health indicators.

```json
{
  "heartbeat_age_seconds": 45,
  "heartbeat_stale": false,
  "telegram_bot_running": true
}
```

## Connection Lifecycle

```
Client                          Server
  |                                |
  |-------- TCP + WS Upgrade ----->|
  |                                |
  |<---------- hello -------------|
  |<---------- snapshot ----------|
  |                                |
  |           (periodic)           |
  |<---------- update ------------|
  |<---------- update ------------|
  |                                |
  |----------- ping ------------->|
  |<---------- pong --------------|
  |                                |
  |--- request_snapshot --------->|
  |<---------- snapshot ----------|
  |                                |
  |-------- close --------------->|
```

## Extensibility

The protocol is designed to be extended:

- New fields can be added to any data object without breaking clients.
  Clients should ignore unknown fields.
- New top-level data sections can be added to `snapshot`/`update` payloads.
- New message types can be introduced; clients should ignore unknown types.
- The `version` field enables future breaking changes with negotiation.

## Running the Server

```bash
# From the lobster source directory:
cd /home/admin/lobster/src/dashboard
/home/admin/lobster/.venv/bin/python3 server.py

# With options:
/home/admin/lobster/.venv/bin/python3 server.py \
  --host 0.0.0.0 \
  --port 9100 \
  --interval 3.0 \
  --log-level INFO
```
