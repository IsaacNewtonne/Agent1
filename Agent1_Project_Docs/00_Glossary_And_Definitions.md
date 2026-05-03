# Glossary and Definitions

## Agent

A configurable AI worker with a name, role, system instruction, model, tools, memory configuration, and permissions.

## Agent Runtime

The Rust engine that loads agents, manages sessions, builds prompts, calls local models, executes tools, handles memory, and emits events.

## Agent Card

A local JSON description of an agent, including its name, description, skills, input modes, output modes, permissions, and endpoint. It acts like a digital business card for agent discovery.

## Agent Skill

A specific capability an agent can perform, such as `review_code`, `write_plan`, `summarize_files`, or `run_tests`.

## A2A-Style Communication

Agent-to-agent communication inspired by open agent collaboration patterns. Agent1 will implement a local-first, simple version first, then evolve toward stronger compatibility.

## MCP

Model Context Protocol. In Agent1, MCP is used to connect agents to external local tools, resources, and workflows through MCP servers.

## MCP Client

The Agent1 component that connects to MCP servers, lists available tools/resources, and exposes them to Agent1 agents through the internal tool registry.

## Tool

A callable function that gives an agent a controlled ability, such as reading files, writing files, querying SQLite, running a command, or calling another agent.

## Tool Registry

The internal registry that stores all available native tools and MCP-provided tools.

## Permission Guard

The component that checks whether an agent is allowed to use a tool or must ask the user for approval.

## Session

A conversation or task execution context. It contains messages, state, tool calls, events, memory updates, and artifacts.

## State

Structured session data used while an agent is running.

## Memory

Local stored context that agents can retrieve across turns or sessions. Agent1 supports short-term session memory and long-term local memory.

## Event

A structured runtime record, such as model response, tool request, tool result, memory write, agent handoff, error, or user approval.

## Artifact

A file or structured output generated during an agent run, such as Markdown, code, JSON, patch files, reports, diagrams, or logs.

## Local Model Provider

A model backend running on the user's machine or private server, such as Ollama, llama.cpp server, vLLM, or another OpenAI-compatible local endpoint.

## Host Agent

The main coordinating agent that receives the user request and decides whether to answer directly or delegate to other agents.

## Worker Agent

An agent that performs a specific subtask.

## Planner Agent

An agent that decomposes a user goal into steps.

## Critic Agent

An agent that reviews the output of other agents.

## Sandbox

A restricted environment or permission boundary where agents can perform limited actions without harming the user's system.
