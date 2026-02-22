//! Lobster Dashboard Protocol types.
//!
//! Defines the JSON message structures received from the Lobster Dashboard
//! WebSocket server. All types derive `Deserialize` for automatic parsing.
//! Fields are kept even if not currently read, for protocol completeness.
//!
//! Also defines outbound message types sent from bisque-computer to the server.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Top-level message frame from the server.
#[derive(Debug, Deserialize, Clone)]
pub struct Frame {
    pub version: String,
    #[serde(rename = "type")]
    pub msg_type: String,
    pub timestamp: String,
    pub data: Option<serde_json::Value>,
}

/// The full dashboard state payload (inside `snapshot` and `update` frames).
#[derive(Debug, Deserialize, Clone, Default)]
pub struct DashboardState {
    #[serde(default)]
    pub system: SystemInfo,
    #[serde(default)]
    pub sessions: Vec<Session>,
    #[serde(default)]
    pub message_queues: MessageQueues,
    #[serde(default)]
    pub tasks: TaskInfo,
    #[serde(default)]
    pub scheduled_jobs: Vec<ScheduledJob>,
    #[serde(default)]
    pub task_outputs: Vec<serde_json::Value>,
    /// Legacy field — kept for backward-compatibility. New field is `memory`.
    #[serde(default)]
    pub recent_memory: Vec<MemoryEvent>,
    #[serde(default)]
    pub subagent_list: SubagentList,
    #[serde(default)]
    pub memory: MemoryStats,
    #[serde(default)]
    pub conversation_activity: ConversationActivity,
    #[serde(default)]
    pub filesystem: Vec<FilesystemEntry>,
    #[serde(default)]
    pub health: Health,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct SystemInfo {
    #[serde(default)]
    pub hostname: String,
    #[serde(default)]
    pub platform: String,
    #[serde(default)]
    pub architecture: String,
    #[serde(default)]
    pub uptime_seconds: u64,
    #[serde(default)]
    pub cpu: CpuInfo,
    #[serde(default)]
    pub memory: MemoryInfo,
    #[serde(default)]
    pub disk: DiskInfo,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct CpuInfo {
    #[serde(default)]
    pub count: u32,
    #[serde(default)]
    pub percent: f64,
    #[serde(default)]
    pub load_avg: Vec<f64>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct MemoryInfo {
    #[serde(default)]
    pub total_mb: u64,
    #[serde(default)]
    pub used_mb: u64,
    #[serde(default)]
    pub available_mb: u64,
    #[serde(default)]
    pub percent: f64,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct DiskInfo {
    #[serde(default)]
    pub total_gb: f64,
    #[serde(default)]
    pub used_gb: f64,
    #[serde(default)]
    pub free_gb: f64,
    #[serde(default)]
    pub percent: f64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Session {
    pub pid: u32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub cmdline: String,
    pub started: Option<String>,
    #[serde(default)]
    pub cpu_percent: f64,
    #[serde(default)]
    pub memory_mb: f64,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct MessageQueues {
    #[serde(default)]
    pub inbox: QueueInfo,
    #[serde(default)]
    pub processing: QueueCount,
    #[serde(default)]
    pub processed: QueueCount,
    #[serde(default)]
    pub sent: QueueCount,
    #[serde(default)]
    pub outbox: QueueCount,
    #[serde(default)]
    pub failed: QueueCount,
    #[serde(default)]
    pub dead_letter: QueueCount,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct QueueInfo {
    #[serde(default)]
    pub count: u64,
    #[serde(default)]
    pub recent: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct QueueCount {
    #[serde(default)]
    pub count: u64,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct TaskInfo {
    #[serde(default)]
    pub tasks: Vec<Task>,
    #[serde(default)]
    pub next_id: u64,
    #[serde(default)]
    pub summary: TaskSummary,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct Task {
    #[serde(default)]
    pub id: u64,
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub status: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct TaskSummary {
    #[serde(default)]
    pub total: u64,
    #[serde(default)]
    pub pending: u64,
    #[serde(default)]
    pub in_progress: u64,
    #[serde(default)]
    pub completed: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ScheduledJob {
    pub name: String,
    #[serde(default)]
    pub file: String,
    #[serde(default)]
    pub size_bytes: u64,
    #[serde(default)]
    pub modified: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MemoryEvent {
    pub id: Option<u64>,
    #[serde(default)]
    pub timestamp: String,
    #[serde(rename = "type", default)]
    pub event_type: String,
    #[serde(default)]
    pub source: String,
    pub project: Option<String>,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub consolidated: bool,
}

// ---------------------------------------------------------------------------
// Subagent types
// ---------------------------------------------------------------------------

/// Runtime statistics parsed from a JSONL task output file.
#[derive(Debug, Deserialize, Clone, Default)]
pub struct AgentRuntime {
    #[serde(default)]
    pub agent_id: String,
    #[serde(default)]
    pub turns: u64,
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub tool_uses: u64,
    #[serde(default)]
    pub top_tools: HashMap<String, u64>,
    pub last_activity_seconds_ago: Option<u64>,
    #[serde(default)]
    pub stale: bool,
    pub first_timestamp: Option<String>,
    pub last_timestamp: Option<String>,
}

/// A single pending Lobster agent with optional runtime stats.
#[derive(Debug, Deserialize, Clone, Default)]
pub struct SubagentInfo {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub description: String,
    pub chat_id: Option<i64>,
    pub started_at: Option<String>,
    pub elapsed_seconds: Option<u64>,
    #[serde(default)]
    pub status: String,
    pub runtime: Option<AgentRuntime>,
}

/// The full subagent list payload from `collect_subagent_list`.
#[derive(Debug, Deserialize, Clone, Default)]
pub struct SubagentList {
    #[serde(default)]
    pub pending_count: u64,
    #[serde(default)]
    pub agents: Vec<SubagentInfo>,
    #[serde(default)]
    pub running_tasks: Vec<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Memory stats types
// ---------------------------------------------------------------------------

/// A canonical memory file (a .md file in memory/canonical/).
#[derive(Debug, Deserialize, Clone, Default)]
pub struct CanonicalFile {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub path: String,
    pub modified: Option<String>,
    #[serde(default)]
    pub size_bytes: u64,
}

/// Consolidation metadata: last run time and list of canonical files.
#[derive(Debug, Deserialize, Clone, Default)]
pub struct MemoryConsolidations {
    pub last_consolidation_at: Option<String>,
    #[serde(default)]
    pub canonical_files: Vec<CanonicalFile>,
}

/// Full memory statistics from `collect_memory_stats`.
#[derive(Debug, Deserialize, Clone, Default)]
pub struct MemoryStats {
    #[serde(default)]
    pub total_events: u64,
    #[serde(default)]
    pub unconsolidated_count: u64,
    #[serde(default)]
    pub event_type_counts: HashMap<String, u64>,
    #[serde(default)]
    pub projects: Vec<String>,
    #[serde(default)]
    pub recent_events: Vec<MemoryEvent>,
    #[serde(default)]
    pub consolidations: MemoryConsolidations,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ConversationActivity {
    #[serde(default)]
    pub messages_received_1h: u64,
    #[serde(default)]
    pub messages_received_24h: u64,
    #[serde(default)]
    pub replies_sent_1h: u64,
    #[serde(default)]
    pub replies_sent_24h: u64,
    #[serde(default)]
    pub failed_1h: u64,
    #[serde(default)]
    pub failed_24h: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FilesystemEntry {
    pub path: String,
    #[serde(default)]
    pub absolute_path: String,
    #[serde(default)]
    pub file_count: u64,
    #[serde(default)]
    pub exists: bool,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct Health {
    pub heartbeat_age_seconds: Option<u64>,
    #[serde(default)]
    pub heartbeat_stale: bool,
    #[serde(default)]
    pub telegram_bot_running: bool,
}

/// Connection status for a single Lobster instance.
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionStatus {
    Connecting,
    Connected,
    Disconnected,
    Error(String),
}

impl Default for ConnectionStatus {
    fn default() -> Self {
        ConnectionStatus::Connecting
    }
}

/// Represents a single Lobster instance connection with its state.
#[derive(Debug, Clone)]
pub struct LobsterInstance {
    pub url: String,
    pub status: ConnectionStatus,
    pub state: DashboardState,
    pub last_update: Option<String>,
    pub protocol_version: Option<String>,
}

impl LobsterInstance {
    pub fn new(url: String) -> Self {
        Self {
            url,
            status: ConnectionStatus::Connecting,
            state: DashboardState::default(),
            last_update: None,
            protocol_version: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Outbound message types (bisque-computer → Lobster server)
// ---------------------------------------------------------------------------

/// A voice transcription result sent to the Lobster server as a user message.
///
/// The server handles this identically to a Telegram text message, routing
/// it through the Lobster assistant pipeline.
#[derive(Debug, Clone, Serialize)]
pub struct VoiceInputMessage {
    pub version: String,
    #[serde(rename = "type")]
    pub msg_type: String,
    pub timestamp: String,
    pub text: String,
    pub source: String,
}

impl VoiceInputMessage {
    /// Construct a new voice input message with the current UTC timestamp.
    pub fn new(text: String) -> Self {
        let timestamp = chrono::Utc::now().to_rfc3339();
        Self {
            version: "1.0".to_string(),
            msg_type: "voice_input".to_string(),
            timestamp,
            text,
            source: "bisque-computer".to_string(),
        }
    }

    /// Serialize to JSON string for transmission.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}
