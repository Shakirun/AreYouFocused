# AreYouFocused

Desktop-first **random-ping** productivity tracker (WhatNow-style): honest, local-first, SQLite. Product charter and Cursor rules live under **`.cursor/`** locally (not in git).

## Development

- **TDD only**: red → green → refactor; failing test before implementation (local rule pack under `.cursor/rules/`).
- **Frontend:** Node 18+, `npm install`, `npm run dev` (Vite + React + **Tailwind**). Design tokens match ui-ux-pro-max design system (see `tailwind.config.js`).

## Agent / Cursor (local workspace only)

Handbook (`AGENTS.md`), `docs/`, and `.cursor/` are **gitignored** — keep them on your machine; they are not part of the shared repo. Copy `.cursor/mcp.json.example` → `.cursor/mcp.json` for MCP when needed.
