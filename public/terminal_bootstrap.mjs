import { Terminal } from "https://esm.sh/@xterm/xterm@5.5.0";
import { FitAddon } from "https://esm.sh/@xterm/addon-fit@0.10.0?deps=@xterm/xterm@5.5.0";
import { WebLinksAddon } from "https://esm.sh/@xterm/addon-web-links@0.11.0?deps=@xterm/xterm@5.5.0";

const instances = new Map();
let nextId = 1;

function readCssVar(name, fallback = "") {
  try {
    const v = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
    return v || fallback;
  } catch (_) {
    return fallback;
  }
}

function xtermThemeFromDom() {
  return {
    background: "rgba(0,0,0,0)",
    foreground: readCssVar("--term-fg", "#f1f2f5"),
    cursor: readCssVar("--term-cursor", "#58a6ff"),
    cursorAccent: readCssVar("--term-cursor", "#58a6ff"),
    selectionBackground: readCssVar("--accent-soft", "rgba(88, 166, 255, 0.30)"),
    black: readCssVar("--bg-app", "#090a0d"),
    red: readCssVar("--danger", "#f85149"),
    green: readCssVar("--success", "#3fb950"),
    yellow: readCssVar("--warning", "#d4a017"),
    blue: readCssVar("--accent", "#58a6ff"),
    magenta: readCssVar("--syntax-keyword", "#f2a3ff"),
    cyan: readCssVar("--syntax-type", "#63e6be"),
    white: readCssVar("--text", "#f1f2f5"),
    brightBlack: readCssVar("--text-faint", "#676c78"),
    brightRed: readCssVar("--danger", "#f85149"),
    brightGreen: readCssVar("--success", "#3fb950"),
    brightYellow: readCssVar("--warning", "#d4a017"),
    brightBlue: readCssVar("--accent-hover", "#7ab8ff"),
    brightMagenta: readCssVar("--syntax-keyword", "#f2a3ff"),
    brightCyan: readCssVar("--accent-cool", "#9bd3ff"),
    brightWhite: readCssVar("--text-bright", "#ffffff"),
  };
}

function applyThemeToAllTerminals() {
  const theme = xtermThemeFromDom();
  for (const rec of instances.values()) {
    try {
      rec.term.options.theme = theme;
      scheduleRefresh(rec);
    } catch (_) {}
  }
}

window.addEventListener("blxcode-theme-changed", applyThemeToAllTerminals);

function forceLayout(rec) {
  try {
    void rec.container.getBoundingClientRect();
    void rec.term.element?.getBoundingClientRect();
  } catch (_) {}
}

function refreshTerminal(rec) {
  try {
    const end = Math.max(0, (rec.term.rows || 1) - 1);
    rec.term.refresh(0, end);
  } catch (_) {}
}

function scheduleRefresh(rec) {
  forceLayout(rec);
  refreshTerminal(rec);
  requestAnimationFrame(() => {
    forceLayout(rec);
    refreshTerminal(rec);
  });
  for (const delay of [0, 16, 50, 120]) {
    window.setTimeout(() => {
      forceLayout(rec);
      refreshTerminal(rec);
    }, delay);
  }
}

function writeB64(rec, b64) {
  if (!b64) return;
  const bin = atob(b64);
  const u8 = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) u8[i] = bin.charCodeAt(i);
  rec.term.write(u8, () => scheduleRefresh(rec));
  scheduleRefresh(rec);
}

function fitTerminal(rec) {
  try {
    forceLayout(rec);
    rec.fit.fit();
    scheduleRefresh(rec);
    const size = {
      rows: rec.term.rows || 0,
      cols: rec.term.cols || 0,
    };
    if (size.rows <= 0 || size.cols <= 0) {
      scheduleZeroSizeRetry(rec);
    }
    return size;
  } catch (_) {
    scheduleZeroSizeRetry(rec);
    return { rows: 0, cols: 0 };
  }
}

function scheduleZeroSizeRetry(rec) {
  if (rec.zeroFitAttempts >= 40) return;
  rec.zeroFitAttempts += 1;
  const delay = Math.min(16 * rec.zeroFitAttempts, 400);
  window.setTimeout(() => {
    const rect = rec.container.getBoundingClientRect();
    if (rect.width <= 1 || rect.height <= 1) {
      scheduleZeroSizeRetry(rec);
      return;
    }
    const size = fitTerminal(rec);
    const termId = [...instances.entries()].find(([, v]) => v === rec)?.[0];
    if (termId != null && size.rows > 0 && size.cols > 0) {
      dispatchPtyResize(termId, rec, size, true);
    }
  }, delay);
}

function dispatchPtyResize(termId, rec, size, force = false) {
  if (size.rows <= 0 || size.cols <= 0) return;
  if (!force && rec.lastRows === size.rows && rec.lastCols === size.cols) return;
  rec.lastRows = size.rows;
  rec.lastCols = size.cols;
  window.dispatchEvent(
    new CustomEvent("blxcode-pty-resize", {
      detail: { termId, rows: size.rows, cols: size.cols },
    }),
  );
}

function schedulePtyResize(termId, rec, size) {
  if (size.rows <= 0 || size.cols <= 0) return;
  rec.pendingRows = size.rows;
  rec.pendingCols = size.cols;
  window.clearTimeout(rec.resizeTimer);
  rec.resizeTimer = window.setTimeout(() => {
    dispatchPtyResize(termId, rec, {
      rows: rec.pendingRows || 0,
      cols: rec.pendingCols || 0,
    });
  }, 120);
}

function requestFit(termId) {
  const rec = instances.get(termId);
  if (!rec) return null;
  const run = () => {
    const rect = rec.container.getBoundingClientRect();
    if (rect.width <= 1 || rect.height <= 1) {
      scheduleZeroSizeRetry(rec);
      return { rows: 0, cols: 0 };
    }
    const size = fitTerminal(rec);
    dispatchPtyResize(termId, rec, size, true);
    scheduleRefresh(rec);
    return size;
  };
  requestAnimationFrame(() => requestAnimationFrame(run));
  for (const delay of [0, 16, 50, 150, 300, 600]) {
    window.setTimeout(run, delay);
  }
  if (rec.lastRows > 0 && rec.lastCols > 0) {
    return { rows: rec.lastRows, cols: rec.lastCols };
  }
  return { rows: 0, cols: 0 };
}

const gridObservers = new Map();

function observeWorkspaceGrid(container, workspaceId) {
  const key = String(workspaceId);
  const prev = gridObservers.get(key);
  if (prev) {
    try {
      prev.disconnect();
    } catch (_) {}
  }
  const notify = () => {
    const rect = container.getBoundingClientRect();
    if (rect.width <= 1 || rect.height <= 1) return;
    window.dispatchEvent(
      new CustomEvent("blxcode-ws-term-grid-ready", {
        detail: { workspaceId },
      }),
    );
  };
  const ro = new ResizeObserver(notify);
  ro.observe(container);
  const parent = container.parentElement;
  if (parent) ro.observe(parent);
  gridObservers.set(key, ro);
  requestAnimationFrame(() => requestAnimationFrame(notify));
  for (const delay of [0, 16, 50, 150, 300, 600]) {
    window.setTimeout(notify, delay);
  }
  return true;
}

function unobserveWorkspaceGrid(workspaceId) {
  const key = String(workspaceId);
  const ro = gridObservers.get(key);
  if (!ro) return;
  try {
    ro.disconnect();
  } catch (_) {}
  gridObservers.delete(key);
}

window.__blxcodeTerminal = {
  create(container) {
    const id = nextId++;
    const term = new Terminal({
      fontFamily: "JetBrains Mono, ui-monospace, monospace",
      fontSize: 12,
      allowTransparency: true,
      theme: xtermThemeFromDom(),
      disableStdin: false,
      rightClickSelectsWord: false,
      scrollback: 5000,
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    const webLinks = new WebLinksAddon((event, uri) => {
      event.preventDefault();
      event.stopPropagation();
      window.dispatchEvent(
        new CustomEvent("blxcode-open-http", {
          bubbles: true,
          detail: { url: uri },
        }),
      );
    });
    term.loadAddon(webLinks);
    term.open(container);

    const dispatchTerminalEvent = (name, extra = {}) => {
      window.dispatchEvent(
        new CustomEvent(name, {
          detail: { termId: id, ...extra },
        }),
      );
    };

    const attachContextMenu = () => {
      const el = term.element;
      if (!el) return;
      el.addEventListener("contextmenu", (e) => {
        e.preventDefault();
        e.stopPropagation();
        if (e.shiftKey) {
          dispatchTerminalEvent("blxcode-terminal-paste-request");
          return;
        }
        const selection = term.getSelection() || "";
        dispatchTerminalEvent("blxcode-terminal-contextmenu", {
          clientX: e.clientX,
          clientY: e.clientY,
          selection,
          hasSelection: selection.length > 0,
        });
      });
    };
    if (term.element) {
      attachContextMenu();
    } else {
      requestAnimationFrame(attachContextMenu);
    }

    term.attachCustomKeyEventHandler((ev) => {
      const key = ev.key;
      const ctrl = ev.ctrlKey || ev.metaKey;
      if (!ctrl) return true;
      if (ev.shiftKey && (key === "C" || key === "c")) {
        const sel = term.getSelection();
        if (sel && sel.length > 0) {
          dispatchTerminalEvent("blxcode-terminal-copy-request", { selection: sel });
          return false;
        }
        return true;
      }
      if (ev.shiftKey && (key === "V" || key === "v")) {
        dispatchTerminalEvent("blxcode-terminal-paste-request");
        return false;
      }
      if (!ev.shiftKey && (key === "C" || key === "c")) {
        const sel = term.getSelection();
        if (sel && sel.length > 0) {
          dispatchTerminalEvent("blxcode-terminal-copy-request", { selection: sel });
          return false;
        }
      }
      return true;
    });

    const rec = {
      term,
      fit,
      container,
      resizeObserver: null,
      resizeTimer: 0,
      lastRows: 0,
      lastCols: 0,
      pendingRows: 0,
      pendingCols: 0,
      zeroFitAttempts: 0,
    };
    term.onData((data) => {
      window.dispatchEvent(
        new CustomEvent("blxcode-pty-input", {
          detail: { termId: id, data },
        }),
      );
    });
    term.onTitleChange((title) => {
      window.dispatchEvent(
        new CustomEvent("blxcode-pty-title", {
          detail: { termId: id, title: String(title || "") },
        }),
      );
    });
    instances.set(id, rec);
    rec.resizeObserver = new ResizeObserver(() => {
      const rect = rec.container.getBoundingClientRect();
      if (rect.width <= 1 || rect.height <= 1) {
        scheduleZeroSizeRetry(rec);
        return;
      }
      const size = fitTerminal(rec);
      schedulePtyResize(id, rec, size);
    });
    rec.resizeObserver.observe(container);
    const observeTarget = container.parentElement ?? container;
    if (observeTarget !== container) {
      rec.resizeObserver.observe(observeTarget);
    }
    requestAnimationFrame(() => {
      requestAnimationFrame(() => requestFit(id));
    });
    return id;
  },
  dispose(termId) {
    const rec = instances.get(termId);
    if (!rec) return;
    try {
      rec.resizeObserver?.disconnect();
    } catch (_) {}
    window.clearTimeout(rec.resizeTimer);
    try {
      rec.term.dispose();
    } catch (_) {}
    instances.delete(termId);
  },
  fit(termId) {
    const rec = instances.get(termId);
    if (!rec) return null;
    return fitTerminal(rec);
  },
  requestFit(termId) {
    return requestFit(termId);
  },
  writeBytesB64(termId, b64) {
    const rec = instances.get(termId);
    if (!rec) return;
    writeB64(rec, b64);
  },
  showFallback(termId, text) {
    const rec = instances.get(termId);
    if (!rec) return;
    rec.term.clear();
    rec.term.reset();
    for (const line of text.split("\n")) {
      rec.term.writeln(line);
    }
    scheduleRefresh(rec);
    try {
      rec.term.options.disableStdin = true;
    } catch (_) {}
  },
  setStdinEnabled(termId, enabled) {
    const rec = instances.get(termId);
    if (!rec) return;
    try {
      rec.term.options.disableStdin = !enabled;
    } catch (_) {}
  },
  getSelection(termId) {
    const rec = instances.get(termId);
    if (!rec) return "";
    try {
      return rec.term.getSelection() || "";
    } catch (_) {
      return "";
    }
  },
  paste(termId, text) {
    const rec = instances.get(termId);
    if (!rec || !text) return;
    try {
      rec.term.paste(text);
    } catch (_) {}
  },
  selectAll(termId) {
    const rec = instances.get(termId);
    if (!rec) return;
    try {
      rec.term.selectAll();
    } catch (_) {}
  },
  clearSelection(termId) {
    const rec = instances.get(termId);
    if (!rec) return;
    try {
      rec.term.clearSelection();
    } catch (_) {}
  },
  focus(termId) {
    const rec = instances.get(termId);
    if (!rec) return;
    try {
      rec.term.focus();
    } catch (_) {}
  },
  observeWorkspaceGrid(container, workspaceId) {
    return observeWorkspaceGrid(container, workspaceId);
  },
  unobserveWorkspaceGrid(workspaceId) {
    unobserveWorkspaceGrid(workspaceId);
  },
};

window.dispatchEvent(new CustomEvent("blxcode-terminal-api-ready"));
