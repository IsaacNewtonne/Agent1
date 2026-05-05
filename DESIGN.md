# Agent1 Design System

Agent1 uses a premium dark orchestration-canvas interface inspired by agent graph boards, neural node maps, and desktop command centers.

## Visual Direction

- Futuristic desktop app, not a generic admin panel.
- Dark glass panels over a subtle cyan grid atmosphere.
- Center canvas is the primary focus: large glowing project core, side lanes, connector lines, and agent nodes.
- Local agents use pink/purple energy; external tools use cyan/blue energy; connected and ready states use teal.
- Glow is selective: status, selected nodes, primary actions, and the project core.

## Layout

- Preserve the product structure: left control panel, central orchestration canvas, right monitoring panel, bottom command composer.
- Side panels should feel lighter than the center canvas and use compact card sections.
- The center canvas owns the visual weight with a large project sphere, lane headers, central ports, branch rails, and graph-like connectors.
- Bottom composer should feel attached to the workflow and should visually connect agent selection, workspace, prompt, and Run Prompt action.

## Typography

- Display: Aptos Display / Sora style, tight tracking for product headings.
- UI: Aptos / Segoe UI Variable style for clear desktop readability.
- Mono: Cascadia Code / Space Mono style for URLs, timestamps, model IDs, counters, and stream output.
- Use uppercase mono eyebrows for system labels and section classification.

## Color Tokens

- Background: near-black navy `#030712`.
- Surfaces: translucent navy layers with blue-tinted borders.
- Primary status accent: teal/cyan `#4de7d4` for connected, ready, and primary execution states.
- Right/external accent: blue `#49c9ff` / `#58c7ff` for external tools and canvas telemetry.
- Left/local accent: purple/pink `#c26bff` / `#f47cff` / `#ff71dc` for local-agent lanes and neural visuals.
- Warning/error: amber `#ffbc5e` and red `#ff6969`, only for running approvals and failures.

## Components

- Panels: glassmorphism, 18-24px radius, thin luminous borders, soft shadows.
- Section cards: clear titles, compact controls, consistent internal padding.
- Agent nodes: card-like, glowing selected state, small status dot, model metadata in mono.
- Empty states: card treatment with small label icon and one direct suggestion.
- Stream output: terminal-like widget with mono text and subtle live cursor.
- Command composer: elevated bottom glass surface with primary Run Prompt button.

## Motion

- Use slow breathing pulses for connected/running status and central core halo.
- Avoid excessive flashy motion; animation should communicate life and readiness.
- Hover should be small: slight lift, border brightening, and shadow only.
