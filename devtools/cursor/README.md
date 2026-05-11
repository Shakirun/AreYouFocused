# Cursor hooks (local)

The repo root **`.cursor/`** is gitignored (solo dev). To auto-inject a follow-up after **`src/App.tsx`** is edited so the agent calls **21st `21st_magic_component_builder`**:

1. Merge `devtools/cursor/hooks.posttooluse-21st.json` into your **`.cursor/hooks.json`** (create the file if missing). If you already have hooks, add the `postToolUse` array entry without removing others.
2. Paths in that snippet assume commands run from the **project root** (`AreYouFocused/`).
3. Restart Cursor or reload hooks (Cursor watches `hooks.json`).

The script **`scripts/cursor/after-app-tsx-21st.cjs`** is committed; it runs on **`postToolUse`** for tools matching `Write|StrReplace` and outputs `additional_context` only when the edited path ends with `src/App.tsx`.

**Limitation:** Hooks cannot call MCP themselves; they only add instructions so the **agent** runs `21st_magic_component_builder` on the next turn.
