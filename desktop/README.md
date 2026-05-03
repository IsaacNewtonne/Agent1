# Agent1 Desktop (React + Tauri)

This folder contains the in-progress desktop shell migration from the static `app/` UI to a React/Tauri stack.

## Prerequisites

- Node.js 20+
- Rust toolchain
- Tauri prerequisites for your OS

## Run (Web UI only)

```powershell
npm install
npm run dev
```

Then open `http://localhost:1420`.

## Run (Tauri Desktop)

```powershell
npm install
npm run tauri:dev
```

The app expects the Agent1 API server to be running at `http://127.0.0.1:17371` by default. Change API base in Settings inside the app.
