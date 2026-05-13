import { Terminal } from "https://esm.sh/@xterm/xterm@5.5.0";
import { FitAddon } from "https://esm.sh/@xterm/addon-fit@0.10.0?deps=@xterm/xterm@5.5.0";

const instances = new Map();
let nextId = 1;

function writeB64(term, b64) {
  if (!b64) return;
  const bin = atob(b64);
  const u8 = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) u8[i] = bin.charCodeAt(i);
  term.write(u8);
}

function fitTerminal(rec) {
  try {
    rec.fit.fit();
    return {
      rows: rec.term.rows || 0,
      cols: rec.term.cols || 0,
    };
  } catch (_) {
    return { rows: 0, cols: 0 };
  }
}

function dispatchPtyResize(termId, rec, size) {
  if (size.rows <= 0 || size.cols <= 0) return;
  if (rec.lastRows === size.rows && rec.lastCols === size.cols) return;
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

window.__blxcodeTerminal = {
  create(container) {
    const id = nextId++;
    const term = new Terminal({
      fontFamily: "JetBrains Mono, ui-monospace, monospace",
      fontSize: 12,
      allowTransparency: true,
      theme: { background: "rgba(0,0,0,0)", foreground: "#e8e8ec" },
      disableStdin: false,
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(container);
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
      const size = fitTerminal(rec);
      schedulePtyResize(id, rec, size);
    });
    rec.resizeObserver.observe(container);
    for (const delay of [0, 50, 150, 300]) {
      window.setTimeout(() => {
        const size = fitTerminal(rec);
        if (delay === 300) dispatchPtyResize(id, rec, size);
      }, delay);
    }
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
  writeBytesB64(termId, b64) {
    const rec = instances.get(termId);
    if (!rec) return;
    writeB64(rec.term, b64);
  },
  showFallback(termId, text) {
    const rec = instances.get(termId);
    if (!rec) return;
    rec.term.clear();
    rec.term.reset();
    for (const line of text.split("\n")) {
      rec.term.writeln(line);
    }
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
};
