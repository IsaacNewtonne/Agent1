# Agent1 Desktop UI Redesign — SPEC.md

## 1. Concept & Vision

A mission-control aesthetic: dark, focused, alive. The interface feels like a command center —
three fixed panels surround a central project canvas where agents branch out like a living system.
Not a dashboard you glance at, but a workspace you inhabit. Every element earns its place.

**Emotional target:** competent, quiet confidence. The UI is the co-pilot, not the showboat.

---

## 2. Design Language

### Aesthetic Direction
**Industrial Mission Control** — dark matte surfaces, subtle scanline texture, glowing status indicators,
space-age typography. Think submarine control room meets modern IDE.

### Color Palette
```
Background base:    #07090f   (near-black with blue tint)
Surface level 1:    #0d1420   (panel backgrounds)
Surface level 2:    #111b2a   (elevated cards)
Border default:     rgba(255,255,255,0.08)
Border active:      rgba(255,255,255,0.14)

Accent Primary:     #2ce0a3   (mint green — local agents, confirm)
Accent Secondary:   #6cb9ff   (sky blue — info, external agents)
Accent Warning:     #ffb347   (amber — running/active state)
Accent Danger:      #ff6d6d   (red — destructive actions)
Accent Purple:      #a78bfa   (violet — Slack MCP, special)

Text Primary:       #f0ece4   (warm white)
Text Secondary:    #8a8478   (muted labels)
Text Mono:         #9ab8d4   (technical labels — Space Mono)

Glow Green:         rgba(44,224,163,0.25)
Glow Blue:          rgba(108,185,255,0.25)
Glow Amber:         rgba(255,179,71,0.25)
```

### Typography
- **UI Copy:** `DM Sans` (400, 500, 600) — humanist, readable
- **Technical/Timestamps/Model names:** `Space Mono` (400, 700) — monospace, precise
- **Display/Headings:** `Sora` (600, 700) — geometric, modern
- Fallbacks: system-ui, sans-serif

### Spatial System
- Left panel width: 220px (fixed)
- Right panel width: 220px (fixed)
- Center canvas: fluid (fill remaining space)
- Panel padding: 16px
- Gap between panels: 12px
- Card/element radius: 10px
- Button radius: 8px

### Motion Philosophy
- **Status pulses:** 2s ease-in-out infinite (ambient life)
- **Agent running state:** 1.5s pulse on amber glow
- **Tree expand/collapse:** 200ms ease-out height transitions
- **Card entrance:** 150ms fade + 6px translateY
- **Button hover:** 120ms transform + border glow
- **Streaming output:** blinking cursor (530ms interval)
- **Scanline overlay:** subtle 2px repeating-linear-gradient

### Visual Assets
- **Icons:** No external icon library — text/emoji glyphs for agents, Unicode symbols for UI
- **Frame shader buttons:** layered gradients + box-shadow glow per color variant
- **Status dots:** 8px circles with matching glow shadows
- **Tree connectors:** 1px border-left with color matching parent node

---

## 3. Layout & Structure

```
┌─────────────────────────────────────────────────────────────────────────┐
│  FULL VIEWPORT — three-panel grid                                       │
│                                                                         │
│  ┌──────────┬─────────────────────────────────────┬──────────────────┐ │
│  │  LEFT    │           CENTER CANVAS              │       RIGHT       │ │
│  │  PANEL   │                                     │       PANEL       │ │
│  │  220px   │         fluid (min 600px)           │       220px       │ │
│  │          │                                     │                   │ │
│  │ Settings │   [Project Icon — center]           │   Activity Feed   │ │
│  │          │        ↙           ↘                 │                   │ │
│  │ • API    │    Local Agents   External Agents   │   • Running task  │ │
│  │ • Refresh│     (branch left)   (branch right)  │   • Stream output │ │
│  │ • Model  │                                     │   • Events        │ │
│  │          │   Task input at bottom              │                   │ │
│  └──────────┴─────────────────────────────────────┴──────────────────┘ │
└─────────────────────────────────────────────────────────────────────────┘
```

### Center Canvas Detail
- **Top center:** Project icon (large, glowing frame shader) with project name
- **Left subtree:** Local agents (from `/api/agents`) branch LEFT, tree expands vertically
- **Right subtree:** External agents / MCP servers (from `/api/mcp/servers`) branch RIGHT
- **Bottom:** Task input bar (textarea + run button)
- **Agent tree:** Each agent node shows initials avatar, status badge (idle/running/active),
  model label. Sub-tools are child nodes with connecting border-left.

### Left Panel — Sections (top to bottom)
1. **Connection status** — Ollama indicator (pulsing dot + URL)
2. **Settings** — API base URL, refresh interval, auto-refresh toggle
3. **Local Agents** — Tab filter (All / Idle / Running), agent list with status icons
4. **Agent Builder** — Collapsed by default, expands for new agent creation

### Right Panel — Sections (top to bottom)
1. **Active Session** — Current running task with progress
2. **Stream Output** — Real-time agent output with blinking cursor
3. **Recent Events** — Last 10 events from WebSocket feed
4. **Pending Approvals** — Approval gates requiring user decision

---

## 4. Features & Interactions

### Agent Tree (Center Canvas Left)
- Click agent → select as active (blue glow border)
- Double-click → open agent detail panel
- Hover → show tooltip with model name and status
- Status states:
  - **Idle:** neutral border, no animation
  - **Running:** amber pulsing glow border, 1.5s infinite
  - **Active:** blue glow border (current task)
- Sub-tools expand on click, collapse on second click (200ms height transition)

### External Agent Tree (Center Canvas Right)
- Same interactions as local agents
- Color-coded server type:
  - Teal: filesystem
  - Blue: git
  - Purple: Slack
  - Gray: other

### Task Input (Center Canvas Bottom)
- Textarea with placeholder: "What should the agents do?"
- `Ctrl+Enter` to submit
- Submit → POST `/api/sessions/run`, show spinner, stream results to right panel
- Cancel button appears while running

### Left Panel — Agent Builder
- "New Agent" button expands form inline (slide down 200ms)
- Fields: ID, Name, Provider (dropdown), Model, System Prompt
- Submit → POST `/api/agents`, toast notification
- Error state: red border on invalid fields, inline error message

### Right Panel — Activity Feed
- Events stream in via WebSocket
- Each event: timestamp (Space Mono), event type badge, payload preview
- Click event → expand full JSON in-place
- Auto-scroll to bottom, unless user has scrolled up
- Blinking cursor indicator when waiting for next event

### Frame Shader Buttons
Four variants:
```
Primary  (blue):   bg gradient + blue border + blue glow shadow
Confirm/Run (green): bg gradient + green border + green glow shadow
Ghost (neutral):   subtle bg + neutral border, no glow
Danger (red):      bg gradient + red border + red glow shadow
```

Hover: lift -1px + border brightens + glow intensifies
Active/pressed: inset shadow (::before gradient shifts)
Disabled: 40% opacity, no hover effects

### Atmosphere
- Scanline texture: `repeating-linear-gradient` 2px lines at 5% opacity over entire viewport
- Base background: radial gradient mesh (3 colors, subtle)
- Pulsing status dots on active agents and connection indicator
- Blinking cursor on streaming output text (530ms on/off)

---

## 5. Component Inventory

### `AgentNode`
- Avatar circle (24px) with initials, colored border matching status
- Status dot (8px) top-right of avatar
- Name (DM Sans 14px 500)
- Model label (Space Mono 11px) below name
- States: idle, running (pulsing amber), active (blue glow)
- Expandable children via border-left tree connector

### `McpServerNode`
- Same as AgentNode but color-coded by type
- Expandable tools list as children
- Disabled state: 50% opacity

### `ProjectIcon`
- 64px circle, frame shader treatment (blue glow)
- SVG icon or emoji in center
- Project name below (Sora 16px 600)
- Subtle floating animation (3s ease-in-out infinite translateY ±3px)

### `TaskInputBar`
- Full-width textarea (4 rows default)
- Character count (Space Mono, right-aligned)
- Run button (green frame shader) right side
- Cancel button (ghost) appears when running
- Loading spinner during submission

### `ActivityEvent`
- Timestamp (Space Mono 11px)
- Event type badge (colored pill)
- Payload preview (truncated, expand on click)
- States: default, expanded, error (red left border)

### `StatusDot`
- 8px circle
- Colors: green (connected), amber (running), red (error), gray (idle)
- Pulsing animation when active/warning

### `PanelSection`
- Header with title + collapse toggle
- Collapsible content area
- Collapsed: header only (32px height)
- Expanded: header + content

### `FrameButton`
- Variants: primary, confirm, ghost, danger
- States: default, hover (lift + glow), active (inset), disabled (dim)
- ::before gradient for top highlight
- ::after box-shadow for outer glow

### `SettingsForm`
- API Base URL input
- Refresh interval (number input, 500ms min)
- Auto-refresh toggle (custom styled checkbox)
- Save button

---

## 6. Technical Approach

### Stack
- React 18 (functional components, hooks)
- Vite (build tool)
- Tauri 2.x (desktop shell)
- Plain CSS with CSS custom properties (no Tailwind, no CSS-in-JS)
- WebSocket for real-time events

### Layout Implementation
- CSS Grid: `grid-template-columns: 220px 1fr 220px`
- Full viewport height: `height: 100vh`, `overflow: hidden` on body
- Panels: `overflow-y: auto` with custom scrollbar styling

### State Management
- React useState for UI state
- Existing API fetch patterns already in App.jsx
- Refs for WebSocket and form references

### API Integration (existing endpoints)
- `GET /api/agents` → left panel local agents
- `GET /api/mcp/servers` → right panel external agents
- `POST /api/sessions/run` → task execution
- `GET /api/sessions/{id}/stream` or WebSocket → real-time output
- `WS /ws/events` → activity feed

### File Structure (planned)
```
src/
  components/
    AgentTree.jsx
    McpTree.jsx
    ProjectCanvas.jsx
    ActivityFeed.jsx
    SettingsPanel.jsx
    TaskInput.jsx
    FrameButton.jsx
    StatusDot.jsx
    PanelSection.jsx
  styles/
    variables.css    (color, typography, spacing tokens)
    layout.css       (grid, panels)
    components.css   (buttons, cards, nodes)
    atmosphere.css   (scanlines, glows, animations)
  App.jsx
  main.jsx
```

### Performance
- Virtualize long agent lists (>50 items)
- Debounce settings auto-save
- Throttle WebSocket event rendering (16ms)
- Use `useMemo` for expensive computations

---

## 7. Implementation Phases

### Phase 1: Layout Foundation
- [ ] CSS Grid three-panel layout
- [ ] Panel structure and scrolling
- [ ] CSS variables (colors, spacing, typography)
- [ ] Base atmosphere (scanlines, background mesh)

### Phase 2: Canvas Core
- [ ] Project icon with floating animation
- [ ] Agent tree layout (left side)
- [ ] MCP tree layout (right side)
- [ ] Tree expand/collapse interactions

### Phase 3: Left Panel
- [ ] Connection status with pulsing dot
- [ ] Settings form
- [ ] Agent list with status filtering
- [ ] Agent builder form (collapsed by default)

### Phase 4: Right Panel
- [ ] Activity feed container
- [ ] Event cards with expand/collapse
- [ ] WebSocket integration
- [ ] Auto-scroll behavior

### Phase 5: Task Execution
- [ ] Task input bar at canvas bottom
- [ ] Run/cancel flow
- [ ] Stream output display with blinking cursor
- [ ] Session status display

### Phase 6: Component Polish
- [ ] Frame shader button system
- [ ] Status dot component with animations
- [ ] Panel section collapse/expand
- [ ] Custom scrollbars

### Phase 7: Integration & Testing
- [ ] Connect all API endpoints
- [ ] WebSocket event handling
- [ ] E2E tests with Playwright
- [ ] Build verification