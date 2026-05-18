// blxcode: OpenCode plugin (cross-platform).
//
// OpenCode does not consume Python hook scripts the way Claude / Codex /
// Gemini / Cursor do — it loads JS/TS plugins from
// `~/.config/opencode/plugin/` and `.opencode/plugin/`. This single
// plugin file handles both responsibilities the Python hooks cover for
// the other agents:
//
//   1. Title rewrite — on every `chat.message`, derive a short topic
//      from the user prompt and emit an OSC-2 sequence on the
//      controlling terminal so blxcode's terminal cell relabels its
//      tab.
//   2. Session capture — on `session.created` / `session.idle`, persist
//      `{terminal_key -> {agent: opencode, session_id, ...}}` into the
//      blxcode-managed `sessions.json`, so blxcode can issue
//      `opencode --session <id>` to resume the precise prior session
//      for that terminal slot on next launch.
//   3. Notifications — on debounced `session.idle`, increment unread in
//      `notifications.json` so the workbench sidebar can show badges.
//
// Required env (set by blxcode when spawning the PTY):
//   BLX_TERMINAL_KEY         composite "<workspace_id>:<slot_id>:<pane_id>"
//   BLX_SESSIONS_PATH        absolute path to sessions.json
//   BLX_NOTIFICATIONS_PATH   absolute path to notifications.json
//
// All file IO and TTY writes are wrapped — a plugin must never crash
// the host CLI.

import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";

const MAX_TOPIC_LEN = 48;
const PREFIX = "opencode";
const NOTIFY_DEBOUNCE_MS = 2000;

let notifyDebounceTimer: ReturnType<typeof setTimeout> | null = null;

function shortenPrompt(input: string): string {
  const firstLine = (input.split(/\r?\n/)[0] ?? "").replace(/\s+/g, " ").trim();
  if (firstLine.length <= MAX_TOPIC_LEN) return firstLine;
  return firstLine.slice(0, MAX_TOPIC_LEN - 1).trimEnd() + "…";
}

function writeOsc(title: string): void {
  const seq = `\x1b]2;${title}\x07`;
  const targets =
    process.platform === "win32" ? ["CONOUT$", "CON"] : ["/dev/tty"];
  for (const target of targets) {
    try {
      fs.writeFileSync(target, seq, { encoding: "utf8" });
      return;
    } catch {
      // try next target
    }
  }
}

function diagLog(sessionsPath: string, line: string): void {
  try {
    const dir = path.dirname(sessionsPath) || ".";
    fs.mkdirSync(dir, { recursive: true });
    const stamp = new Date().toISOString().replace(/\.\d+Z$/, "Z");
    fs.appendFileSync(
      path.join(dir, "sessions.log"),
      `${stamp} ${line}\n`,
      { encoding: "utf8" },
    );
  } catch {
    // ignore
  }
}

function loadSessions(p: string): {
  version: number;
  terminals: Record<string, unknown>;
} {
  try {
    const raw = fs.readFileSync(p, { encoding: "utf8" });
    const parsed = JSON.parse(raw);
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
      const terminals =
        parsed.terminals && typeof parsed.terminals === "object"
          ? (parsed.terminals as Record<string, unknown>)
          : {};
      return { version: 1, terminals };
    }
  } catch {
    // missing / corrupt — start fresh
  }
  return { version: 1, terminals: {} };
}

function atomicWrite(p: string, value: unknown): void {
  const dir = path.dirname(p) || ".";
  fs.mkdirSync(dir, { recursive: true });
  const tmp = path.join(
    dir,
    `.sessions-${process.pid}-${Date.now()}.tmp`,
  );
  try {
    fs.writeFileSync(tmp, JSON.stringify(value, null, 2), { encoding: "utf8" });
    fs.renameSync(tmp, p);
  } catch {
    try {
      fs.unlinkSync(tmp);
    } catch {
      // ignore
    }
  }
}

function recordSession(
  sessionId: string,
  cwdHint: string | undefined,
): void {
  const terminalKey = (process.env.BLX_TERMINAL_KEY ?? "").trim();
  const sessionsPath = (process.env.BLX_SESSIONS_PATH ?? "").trim();
  if (!terminalKey || !sessionsPath) {
    diagLog(
      sessionsPath || path.join(os.homedir(), ".cache", "blxcode-sessions.log"),
      `opencode SKIP missing_env terminal_key=${Boolean(terminalKey)} sessions_path=${Boolean(sessionsPath)}`,
    );
    return;
  }
  const id = (sessionId ?? "").trim();
  if (!id) return;

  diagLog(sessionsPath, `opencode WRITE key=${terminalKey} session_id=${id}`);
  const state = loadSessions(sessionsPath);
  state.terminals[terminalKey] = {
    agent: "opencode",
    session_id: id,
    cwd: cwdHint ?? process.cwd(),
    source: "startup",
    updated_at: new Date().toISOString().replace(/\.\d+Z$/, "Z"),
  };
  atomicWrite(sessionsPath, state);
}

function loadNotifications(p: string): {
  version: number;
  terminals: Record<string, { unread?: number; agent?: string; updated_at?: string }>;
} {
  try {
    const raw = fs.readFileSync(p, { encoding: "utf8" });
    const parsed = JSON.parse(raw);
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
      const terminals =
        parsed.terminals && typeof parsed.terminals === "object"
          ? (parsed.terminals as Record<
              string,
              { unread?: number; agent?: string; updated_at?: string }
            >)
          : {};
      return { version: 1, terminals };
    }
  } catch {
    // missing / corrupt
  }
  return { version: 1, terminals: {} };
}

function recordNotification(): void {
  const terminalKey = (process.env.BLX_TERMINAL_KEY ?? "").trim();
  const notificationsPath = (process.env.BLX_NOTIFICATIONS_PATH ?? "").trim();
  if (!terminalKey || !notificationsPath) return;

  const agent = (process.env.BLX_AGENT_SLUG ?? "opencode").trim() || "opencode";
  const state = loadNotifications(notificationsPath);
  const prev = state.terminals[terminalKey] ?? {};
  const unread =
    typeof prev.unread === "number"
      ? prev.unread
      : Number.parseInt(String(prev.unread ?? "0"), 10) || 0;
  state.terminals[terminalKey] = {
    unread: unread + 1,
    agent,
    updated_at: new Date().toISOString().replace(/\.\d+Z$/, "Z"),
  };
  atomicWrite(notificationsPath, state);
}

function scheduleNotification(): void {
  if (notifyDebounceTimer) {
    clearTimeout(notifyDebounceTimer);
  }
  notifyDebounceTimer = setTimeout(() => {
    notifyDebounceTimer = null;
    try {
      recordNotification();
    } catch {
      // ignore
    }
  }, NOTIFY_DEBOUNCE_MS);
}

// OpenCode plugin entry. The exact `Plugin` type isn't imported (avoids
// requiring `@opencode-ai/plugin` as a dev dependency in this repo);
// OpenCode treats the default export as a plugin factory.
export const BlxcodePlugin = async (ctx: {
  directory?: string;
  project?: { worktree?: string };
}) => {
  const cwdHint = ctx?.project?.worktree ?? ctx?.directory;

  return {
    event: async ({ event }: { event: { type: string } & Record<string, unknown> }) => {
      try {
        const sessionId =
          (event as { session_id?: string }).session_id ??
          (event as { sessionID?: string }).sessionID;
        if (event.type === "session.created" && typeof sessionId === "string") {
          recordSession(sessionId, cwdHint);
        } else if (event.type === "session.idle" && typeof sessionId === "string") {
          recordSession(sessionId, cwdHint);
          scheduleNotification();
        }
      } catch {
        // never break the host CLI
      }
    },

    "chat.message": async (input: {
      message?: { parts?: Array<{ type?: string; text?: string }> };
    }) => {
      try {
        const parts = input?.message?.parts ?? [];
        const textPart = parts.find(
          (p) => p && p.type === "text" && typeof p.text === "string",
        );
        const text = textPart?.text ?? "";
        const topic = shortenPrompt(text);
        if (topic) {
          writeOsc(`${PREFIX} · ${topic}`);
        }
      } catch {
        // ignore
      }
    },
  };
};

export default BlxcodePlugin;
