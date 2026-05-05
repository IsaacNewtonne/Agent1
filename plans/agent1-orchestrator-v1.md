# Agent1 Orchestrator Platform — Architectural Blueprint

## 1. Concept & Vision

Agent1 transforms from a simple agent runtime into a **chief-of-staff-level orchestrator** — an intelligent, autonomous agent that breaks down complex objectives into execution plans, dynamically builds specialized agent teams, coordinates their work, and reports back to the user only when human judgment is required.

The experience should feel like delegating a goal to a highly competent executive operator, not interacting with a chatbot.

---

## 2. Architecture Overview

### 2.1 System Layers

```
┌─────────────────────────────────────────────────────────────┐
│                     USER INTERACTION LAYER                   │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────────┐ │
│  │ Desktop UI   │  │ WhatsApp     │  │ CLI / Terminal     │ │
│  │ (Tauri)      │  │ Integration  │  │                    │ │
│  └──────────────┘  └──────────────┘  └────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
                              │
                    ┌─────────┴─────────┐
                    │  ORCHESTRATOR CORE │
                    │                    │
                    │ • Goal Decomposer  │
                    │ • Team Manager     │
                    │ • Progress Tracker │
                    │ • Escalation Mgmt  │
                    │ • Session Coord.   │
                    └─────────┬──────────┘
                              │
┌─────────────────────────────┴───────────────────────────────┐
│                    AGENT EXECUTION LAYER                    │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │ Planner      │  │ Worker       │  │ Critic           │  │
│  │ Agent        │  │ Agents       │  │ Agent            │  │
│  └──────────────┘  └──────────────┘  └──────────────────┘  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │ Researcher   │  │ Builder      │  │ Custom Agents    │  │
│  │ Agent        │  │ Agent        │  │ (user-defined)   │  │
│  └──────────────┘  └──────────────┘  └──────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────┴───────────────────────────────┐
│                      RUNTIME LAYER                          │
│  ┌──────────────────┐  ┌────────────────┐  ┌────────────┐  │
│  │ AgentRuntime     │  │ ToolRegistry   │  │ SQLite DB  │  │
│  │ (existing)       │  │ (existing)     │  │ (existing) │  │
│  └──────────────────┘  └────────────────┘  └────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 Core Components

#### Orchestrator Core (`agent1-orchestrator` crate)

The central orchestrator that manages the entire execution lifecycle:

```
agent1-orchestrator/
├── src/
│   ├── lib.rs              # Main orchestrator entry
│   ├── orchestrator.rs     # Core orchestration logic
│   ├── goal_decomposer.rs  # Break goals into plans
│   ├── team_manager.rs     # Agent lifecycle & team coordination
│   ├── progress_tracker.rs # Track & report progress
│   ├── escalation.rs       # User escalation decisions
│   ├── session_coord.rs    # Coordinate multi-agent sessions
│   ├── types.rs            # Orchestrator-specific types
│   └── prompts.rs          # Orchestrator prompts
```

#### Agent Types (extensions to existing `agent1-core`)

New agent roles beyond the basic `Agent`:

```rust
// New in agent1-core
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentRole {
    Orchestrator,  // Agent1 itself - top level
    Planner,       // Breaks down goals into steps
    Worker,        // Executes specific tasks
    Critic,        // Reviews and quality-checks work
    Researcher,    // Gathers information
    Builder,       // Creates artifacts (code, docs, etc.)
    Reporter,      // Compiles status reports
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub id: String,
    pub objective: String,
    pub steps: Vec<ExecutionStep>,
    pub parent_plan_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub status: PlanStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStep {
    pub id: String,
    pub description: String,
    pub assigned_agent_id: Option<String>,
    pub assigned_role: Option<AgentRole>,
    pub dependencies: Vec<String>,  // Step IDs this depends on
    pub status: StepStatus,
    pub output: Option<String>,
    pub review_notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepStatus {
    Pending,
    InProgress,
    Completed,
    Blocked,
    NeedsReview,
    Failed,
}
```

---

## 3. Execution Flow

### 3.1 Goal Handling Flow

```
User Input: "build a customer portal for my SaaS"
         │
         ▼
┌─────────────────────────────────────┐
│     AGENT1 ORCHESTRATOR             │
│                                     │
│  1. GOAL RECEIVED                   │
│     - Log incoming objective         │
│     - Check for escalation triggers │
│     - Create orchestration session  │
└─────────────────┬───────────────────┘
                  │
                  ▼
┌─────────────────────────────────────┐
│     GOAL DECOMPOSITION              │
│                                     │
│  2. Decompose into execution plan   │
│     • Identify required agents       │
│     • Define dependencies            │
│     • Estimate complexity/risk       │
│     • Determine review checkpoints   │
└─────────────────┬───────────────────┘
                  │
                  ▼
┌─────────────────────────────────────┐
│     TEAM BUILDING                   │
│                                     │
│  3. Create agent team               │
│     • Create Planner agent           │
│     • Create Worker agents           │
│     • Create Critic agent            │
│     • Assign roles & responsibilities│
│     • Define reporting structure    │
└─────────────────┬───────────────────┘
                  │
                  ▼
┌─────────────────────────────────────┐
│     EXECUTION COORDINATION           │
│                                     │
│  4. Execute plan                    │
│     • Assign steps to agents        │
│     • Monitor progress               │
│     • Handle dependencies            │
│     • Collect outputs               │
│     • Trigger reviews               │
└─────────────────┬───────────────────┘
                  │
                  ▼
┌─────────────────────────────────────┐
│     QUALITY ASSURANCE               │
│                                     │
│  5. Review outputs                 │
│     • Critic reviews each step       │
│     • Refine if needed              │
│     • User approval at checkpoints  │
└─────────────────┬───────────────────┘
                  │
                  ▼
┌─────────────────────────────────────┐
│     COMPLETION                      │
│                                     │
│  6. Compile final deliverable       │
│     • Aggregate all outputs         │
│     • Generate summary report       │
│     • Return to user                │
└─────────────────────────────────────┘
```

### 3.2 Multi-Agent Coordination

When a worker agent completes a step:

```
Worker completes Step A
         │
         ▼
┌─────────────────────────────────────┐
│   Dependency Check                  │
│                                     │
│   Are steps B, C waiting on A?      │
│         │                           │
│    YES   │   NO                     │
│    ┌────┘   └────┐                  │
│    ▼            ▼                  │
│ ┌──────────┐  ┌──────────┐          │
│ │ Unblock  │  │ Log to   │          │
│ │ B, C     │  │ progress │          │
│ └────┬─────┘  └──────────┘          │
│      │                              │
│      ▼                              │
│ ┌─────────────────────────────────┐ │
│ │  Assign B, C to workers         │ │
│ │  Start execution               │ │
│ └─────────────────────────────────┘ │
└─────────────────────────────────────┘
```

---

## 4. Escalation Model

Agent1 escalates to the user for:

| Category | Examples | Escalation Trigger |
|----------|----------|-------------------|
| **Security** | API keys, passwords, credentials | Any tool call involving secrets |
| **Finance** | Payments, billing, purchases | `payment`, `purchase`, `subscribe` |
| **Access** | Account connections, OAuth | `connect_account`, `authenticate` |
| **Identity** | Email, phone, personal info | `send_email`, `create_account` |
| **Approvals** | High-risk operations | RiskLevel::High tool calls |
| **External** | Third-party permissions | User-owned resources |

### Escalation Flow

```
Tool call with escalation flag
         │
         ▼
┌─────────────────────────────────────┐
│   Check Escalation Rules            │
│                                     │
│   • Is this a security tool?        │
│   • Does it involve user assets?    │
│   • Is risk level HIGH?            │
│         │                           │
│    YES   │   NO                     │
│    ┌────┘   └────┐                  │
│    ▼            ▼                  │
│ ┌──────────┐  ┌──────────┐          │
│ │ PAUSE    │  │ Execute  │          │
│ │ Request  │  │ normally │          │
│ │ approval │  └──────────┘          │
│ └────┬─────┘                        │
│      │                              │
│      ▼                              │
│ ┌─────────────────────────────────┐ │
│ │  Notify user (UI + WhatsApp)    │ │
│ │  Wait for response              │ │
│ └─────────────────────────────────┘ │
└─────────────────────────────────────┘
```

---

## 5. WhatsApp Integration

### 5.1 Connection Flow

```
User clicks "Connect WhatsApp"
         │
         ▼
┌─────────────────────────────────────┐
│   Generate QR Code                  │
│                                     │
│   • Use whatsapp-web.js library     │
│   • Generate QR as base64 image     │
│   • Display in UI + send via API    │
└─────────────────┬───────────────────┘
                  │
                  ▼
┌─────────────────────────────────────┐
│   User Scans with WhatsApp          │
│                                     │
│   • WhatsApp Web authenticates      │
│   • Session saved to database       │
│   • Connection status updated      │
└─────────────────┬───────────────────┘
                  │
                  ▼
┌─────────────────────────────────────┐
│   Active Connection                 │
│                                     │
│   • Incoming messages → webhook     │
│   • Parse and route to orchestrator │
│   • Outgoing via WhatsApp API       │
└─────────────────────────────────────┘
```

### 5.2 Message Routing

```
WhatsApp Message Received
         │
         ▼
┌─────────────────────────────────────┐
│   Message Parser                    │
│                                     │
│   • Extract sender, content         │
│   • Identify message type           │
│   • Route to appropriate handler   │
└─────────────────┬───────────────────┘
                  │
         ┌────────┼────────┐
         │        │        │
         ▼        ▼        ▼
    ┌────────┐ ┌────────┐ ┌────────┐
    │ Status │ │ New    │ │ Approval│
    │ Query  │ │ Task   │ │ Response│
    └────┬───┘ └───┬────┘ └───┬────┘
         │        │        │
         ▼        ▼        ▼
    ┌─────────────────────────────────┐
    │   Orchestrator Handler          │
    │   (same as desktop/CLI input)   │
    └─────────────────────────────────┘
```

---

## 6. Database Schema Extensions

```sql
-- New tables for orchestrator

CREATE TABLE orchestration_sessions (
    id TEXT PRIMARY KEY,
    objective TEXT NOT NULL,
    plan_id TEXT,
    status TEXT NOT NULL DEFAULT 'running',
    user_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    completed_at TEXT,
    FOREIGN KEY (plan_id) REFERENCES execution_plans(id)
);

CREATE TABLE execution_plans (
    id TEXT PRIMARY KEY,
    orchestration_session_id TEXT NOT NULL,
    objective TEXT NOT NULL,
    raw_goal TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft',
    created_at TEXT NOT NULL,
    completed_at TEXT,
    FOREIGN KEY (orchestration_session_id) REFERENCES orchestration_sessions(id)
);

CREATE TABLE execution_steps (
    id TEXT PRIMARY KEY,
    plan_id TEXT NOT NULL,
    description TEXT NOT NULL,
    step_order INTEGER NOT NULL,
    assigned_agent_id TEXT,
    dependencies TEXT,  -- JSON array of step IDs
    status TEXT NOT NULL DEFAULT 'pending',
    output TEXT,
    review_notes TEXT,
    created_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT,
    FOREIGN KEY (plan_id) REFERENCES execution_plans(id),
    FOREIGN KEY (assigned_agent_id) REFERENCES agents(id)
);

CREATE TABLE agent_instances (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    orchestration_session_id TEXT NOT NULL,
    role TEXT NOT NULL,
    parent_instance_id TEXT,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TEXT NOT NULL,
    terminated_at TEXT,
    FOREIGN KEY (agent_id) REFERENCES agents(id),
    FOREIGN KEY (orchestration_session_id) REFERENCES orchestration_sessions(id),
    FOREIGN KEY (parent_instance_id) REFERENCES agent_instances(id)
);

CREATE TABLE escalation_queue (
    id TEXT PRIMARY KEY,
    orchestration_session_id TEXT NOT NULL,
    step_id TEXT,
    escalation_type TEXT NOT NULL,
    description TEXT NOT NULL,
    payload TEXT,  -- JSON with details
    status TEXT NOT NULL DEFAULT 'pending',
    response TEXT,
    created_at TEXT NOT NULL,
    resolved_at TEXT,
    FOREIGN KEY (orchestration_session_id) REFERENCES orchestration_sessions(id),
    FOREIGN KEY (step_id) REFERENCES execution_steps(id)
);

CREATE TABLE whatsapp_sessions (
    id TEXT PRIMARY KEY,
    phone_number TEXT,
    display_name TEXT,
    session_data TEXT,  -- Encrypted WhatsApp session
    connected_at TEXT NOT NULL,
    last_seen_at TEXT,
    status TEXT NOT NULL DEFAULT 'active'
);
```

---

## 7. Implementation Phases

### Phase 1: Foundation (Weeks 1-2)

**Goal:** Build the orchestrator core without WhatsApp

#### Step 1.1: Create `agent1-orchestrator` crate
- [ ] New Rust crate `agent1-orchestrator`
- [ ] Move/add orchestrator types to `agent1-core`
- [ ] Basic orchestrator struct with goal input handling

#### Step 1.2: Goal Decomposition Engine
- [ ] `GoalDecomposer` struct
- [ ] LLM-powered goal breakdown
- [ ] Plan generation with steps and dependencies
- [ ] Prompt templates for planning

#### Step 1.3: Team Manager
- [ ] `TeamManager` for agent lifecycle
- [ ] `AgentFactory` to create typed agents
- [ ] Agent role definitions (Planner, Worker, Critic, etc.)
- [ ] Parent-child agent relationships

#### Step 1.4: Basic Orchestrator Loop
- [ ] `Orchestrator::run(objective)` entry point
- [ ] Decompose → Plan → Execute loop
- [ ] Progress tracking
- [ ] Simple sequential execution

**Verification:**
```bash
cargo test -p agent1-orchestrator
```

### Phase 2: Multi-Agent Coordination (Weeks 2-3)

**Goal:** Full parallel execution with dependencies

#### Step 2.1: Dependency-Aware Execution
- [ ] Topological sort of execution steps
- [ ] Parallel step execution when dependencies allow
- [ ] Step blocking/unblocking logic

#### Step 2.2: Progress Tracking System
- [ ] Real-time progress updates
- [ ] Progress events to database
- [ ] Progress polling API endpoints

#### Step 2.3: Review & Refinement Loop
- [ ] Critic agent review after each step
- [ ] Automatic refinement on poor output
- [ ] Human review checkpoints

#### Step 2.4: Escalation System
- [ ] Escalation trigger detection
- [ ] Queue management
- [ ] User notification
- [ ] Resolution handling

**Verification:**
```bash
cargo test -p agent1-orchestrator
cargo run --bin agent1 -- orchestrate "build a web app"
```

### Phase 3: WhatsApp Integration (Week 3-4)

**Goal:** Connect WhatsApp as a user interaction channel

#### Step 3.1: WhatsApp Session Management
- [ ] `whatsapp-web.js` integration
- [ ] QR code generation/display API
- [ ] Session persistence to database
- [ ] Reconnection handling

#### Step 3.2: Message Handling
- [ ] Incoming message webhook
- [ ] Message parsing and classification
- [ ] Command recognition

#### Step 3.3: Notification System
- [ ] Progress updates via WhatsApp
- [ ] Escalation notifications
- [ ] Response routing back to user

**Verification:**
```bash
curl -X POST /api/whatsapp/connect  # Returns QR code
curl -X POST /api/whatsapp/webhook   # Test message handling
```

### Phase 4: Enhanced UI (Weeks 4-5)

**Goal:** Full orchestration dashboard

#### Step 4.1: Orchestration Dashboard
- [ ] Active orchestration overview
- [ ] Plan visualization (dependency graph)
- [ ] Agent team view
- [ ] Real-time progress

#### Step 4.2: Interactive Controls
- [ ] Pause/resume orchestration
- [ ] Manual step assignment
- [ ] Intervention controls
- [ ] Escalation response UI

#### Step 4.3: History & Audit
- [ ] Past orchestration sessions
- [ ] Step-by-step playback
- [ ] Export reports

**Verification:**
```bash
cd desktop && npm run tauri:dev
# Navigate to Orchestration tab
# See active plans and progress
```

### Phase 5: Polish & Production (Week 5-6)

**Goal:** Production-ready with error handling

#### Step 5.1: Error Handling
- [ ] Graceful degradation
- [ ] Retry logic
- [ ] Recovery from failures

#### Step 5.2: Performance
- [ ] Concurrent agent limits
- [ ] Resource management
- [ ] Caching

#### Step 5.3: Security
- [ ] WhatsApp session encryption
- [ ] API authentication
- [ ] Input sanitization

**Verification:**
```bash
cargo test --all
npm run build --workspace
```

---

## 8. File Structure

```
agent1/
├── Cargo.toml                  # Workspace
├── crates/
│   ├── agent1-core/           # Core types (extended)
│   ├── agent1-runtime/        # Agent execution (existing)
│   ├── agent1-models/         # Model providers (existing)
│   ├── agent1-tools/          # Tool registry (existing)
│   ├── agent1-db/             # Persistence (extended)
│   ├── agent1-cli/            # CLI & API (extended)
│   ├── agent1-orchestrator/   # NEW: Orchestrator core
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── orchestrator.rs
│   │       ├── goal_decomposer.rs
│   │       ├── team_manager.rs
│   │       ├── progress_tracker.rs
│   │       ├── escalation.rs
│   │       └── types.rs
│   └── agent1-whatsapp/        # NEW: WhatsApp integration
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── session.rs
│           ├── message_handler.rs
│           └── qr.rs
├── desktop/                    # Tauri desktop app (extended)
│   ├── src/
│   │   ├── App.jsx            # Extended with orchestration UI
│   │   └── components/
│   │       ├── OrchestrationDashboard.jsx
│   │       ├── PlanGraph.jsx
│   │       ├── AgentTeamView.jsx
│   │       ├── WhatsAppConnect.jsx
│   │       └── EscalationModal.jsx
│   └── src-tauri/
│       └── src/main.rs
├── plans/                      # This blueprint
│   └── agent1-orchestrator-v1.md
└── SPEC.md                     # Updated specification
```

---

## 9. API Extensions

### New Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/orchestrate` | Start new orchestration |
| GET | `/api/orchestrate/{id}` | Get orchestration status |
| GET | `/api/orchestrate/{id}/plan` | Get execution plan |
| GET | `/api/orchestrate/{id}/steps` | Get all steps |
| POST | `/api/orchestrate/{id}/pause` | Pause orchestration |
| POST | `/api/orchestrate/{id}/resume` | Resume orchestration |
| POST | `/api/escalation/{id}/respond` | Respond to escalation |
| GET | `/api/agents/roles` | List agent roles |
| POST | `/api/whatsapp/connect` | Get QR code |
| GET | `/api/whatsapp/status` | Connection status |
| DELETE | `/api/whatsapp/disconnect` | Disconnect |

### WebSocket Events (Extended)

| Event | Payload | Description |
|-------|---------|-------------|
| `orchestration_start` | `{id, objective}` | New orchestration started |
| `step_update` | `{plan_id, step_id, status}` | Step status changed |
| `escalation_created` | `{id, type, description}` | New escalation |
| `agent_created` | `{instance_id, role}` | New agent instance |
| `orchestration_complete` | `{id, summary}` | Done |

---

## 10. Dependencies

```toml
# New dependencies to add

[dependencies]
# In agent1-orchestrator
serde_json = "1.0"
tokio = { version = "1.0", features = ["full"] }
tracing = "0.1"

# In agent1-whatsapp
whatsapp-web = "0.9"  # or similar library
qrcode = "0.13"
image = "0.25"

[target.'cfg(not(any(target_os = "android", target_os = "ios")))'.dependencies]
# Desktop WhatsApp (not mobile)
```

---

## 11. Exit Criteria

### Phase 1 Complete When:
- [ ] `cargo run --bin agent1 -- orchestrate "hello"` creates a plan
- [ ] Plan has at least a greeting step
- [ ] Can list active orchestrations

### Phase 2 Complete When:
- [ ] Complex goal produces multi-step plan
- [ ] Steps execute in dependency order
- [ ] Critic reviews trigger refinement
- [ ] Escalations pause execution and notify

### Phase 3 Complete When:
- [ ] QR code displays in browser
- [ ] Can scan and authenticate WhatsApp
- [ ] Messages route to orchestrator
- [ ] Notifications send to WhatsApp

### Phase 4 Complete When:
- [ ] Dashboard shows active orchestration
- [ ] Plan graph visualizes dependencies
- [ ] Pause/resume works
- [ ] Can respond to escalations from UI

### Production Ready When:
- [ ] All tests pass
- [ ] No panics in runtime
- [ ] Graceful error messages
- [ ] Documentation complete