/**
 * Cursor postToolUse hook: after Write/StrReplace touches src/App.tsx, inject
 * instructions so the agent calls 21st_magic_component_builder (MCP cannot be
 * invoked from Node — only follow-up context for the agent).
 *
 * Configure in .cursor/hooks.json (see devtools/cursor/hooks.posttooluse-21st.json).
 */
"use strict";

const fs = require("fs");

const raw = fs.readFileSync(0, "utf8");
let input = {};
try {
  input = JSON.parse(raw);
} catch {
  process.stdout.write("{}\n");
  process.exit(0);
}

const tool = String(input.tool_name || "");
const tiRaw = input.tool_input;
let ti = tiRaw;
if (typeof tiRaw === "string") {
  try {
    ti = JSON.parse(tiRaw);
  } catch {
    ti = {};
  }
}
if (!ti || typeof ti !== "object") ti = {};

function pickPath(obj) {
  return (
    obj.path ||
    obj.file_path ||
    obj.target_file ||
    obj.absolutePath ||
    ""
  );
}

const filePath = String(pickPath(ti)).replace(/\\/g, "/");
const isAppTsx = /(^|\/)src\/App\.tsx$/i.test(filePath);
const isWriteLike = /^(Write|StrReplace)$/i.test(tool);

if (!isWriteLike || !isAppTsx) {
  process.stdout.write("{}\n");
  process.exit(0);
}

const root =
  (Array.isArray(input.workspace_roots) && input.workspace_roots[0]) ||
  input.cwd ||
  "";
const abs = pickPath(ti) || (root ? `${root.replace(/\\/g, "/")}/src/App.tsx` : "");

const msg = [
  "MANDATORY follow-up (project hook): `src/App.tsx` was just edited via `" + tool + "`.",
  "",
  "Immediately call MCP `21st_magic_component_builder` on server `user-@21st-dev/magic` with:",
  "- message: short summary of what changed in App.tsx",
  '- searchQuery: 2–4 words, e.g. "minimal capture form"',
  "- absolutePathToCurrentFile: " + abs,
  "- absolutePathToProjectDirectory: " + String(root),
  "- standaloneRequestQuery: Tailwind AreYouFocused tokens — bg-surface #F0FDFA, text-ink #134E4A, bg-brand #0D9488, bg-action #F97316, font Inter, micro-interactions 150–200ms, no emoji icons, Lucide if icons needed, form a11y (label, aria-live), prefers-reduced-motion; keep existing Tauri `invoke` wiring intact.",
  "",
  "Then merge the returned snippet into `src/App.tsx` (adapt imports if 21st suggests shadcn paths).",
].join("\n");

process.stdout.write(
  JSON.stringify({ additional_context: msg }) + "\n"
);
process.exit(0);
