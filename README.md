# AreYouFocused

Desktop-first **random-ping** productivity tracker (WhatNow-style): honest, local-first, SQLite. Product charter and Cursor rules live under **`.cursor/`** locally (not in git).

**Language policy:** All **public repo docs**, **in-app UI copy**, and **code comments** are **English** by default. Planned **i18n** for additional locales is described in [I18N.md](I18N.md).

## Development

- **TDD only**: red → green → refactor; failing test before implementation (local rule pack under `.cursor/rules/`).
- **Frontend:** Node 18+, `npm install`, `npm run dev` (Vite + React + **Tailwind**). Design tokens follow the ui-ux-pro-max design system (see `tailwind.config.js`).
- **Cursor + 21st.dev:** after edits to `src/App.tsx`, an optional hook injects a follow-up to run `21st_magic_component_builder` — see `devtools/cursor/README.md` (`.cursor/` is local-only).

## Git branches

Integrate on **`develop`** (unstable `*-dev.*` versions); ship stable releases via MR **`develop` → `main`**. Details: [RELEASING.md](RELEASING.md).

## Agent / Cursor (local workspace only)

Handbook (`AGENTS.md`), `docs/`, and `.cursor/` are **gitignored** — keep them on your machine; they are not part of the shared repo. Copy `.cursor/mcp.json.example` → `.cursor/mcp.json` for MCP when needed.
