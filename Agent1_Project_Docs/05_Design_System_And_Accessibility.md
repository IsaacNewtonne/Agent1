# Design System and Accessibility

## Visual Style

Agent1 should feel technical, clean, and trustworthy.

## Design Principles

- Clarity over decoration
- Visible state
- Obvious permissions
- Low visual clutter
- High contrast
- Keyboard-friendly
- Developer-focused

## Color Roles

Use semantic colors:

| Role | Use |
|---|---|
| Background | App base |
| Surface | Cards and panels |
| Primary | Main actions |
| Warning | Approval/risk |
| Danger | Destructive actions |
| Success | Completed actions |
| Muted | Secondary text |
| Border | Panel separation |

Do not rely only on color to communicate risk.

## Typography

- Use clear sans-serif UI font.
- Use monospace for code, commands, JSON, logs, and tool inputs.
- Keep line length readable.

## Component Requirements

### Buttons

States:

- Default
- Hover
- Active
- Disabled
- Loading

### Approval Cards

Must include:

- Tool name
- Agent name
- Exact requested action
- Risk label
- Clear approve/deny actions

### Event Items

Must include:

- Event type
- Timestamp
- Agent
- Expandable details

### Agent Nodes

Must include:

- Agent name
- Role
- Status
- Active/inactive state

## Accessibility

- Keyboard navigation for all controls.
- Focus indicators.
- Screen reader labels.
- Color contrast target: WCAG AA.
- Modals trap focus.
- Escape closes non-critical modals.
- Approval modals must not be dismissible without decision unless run is paused.

## Motion

Use minimal motion only for:

- Streaming output
- Active agent pulse
- Event arrival
- Loading states

Avoid distracting animations.
