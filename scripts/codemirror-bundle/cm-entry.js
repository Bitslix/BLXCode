// Entry for the vendored CodeMirror 6 bundle used by the BLXCode in-app editor
// (edit mode). esbuild bundles this into ../../public/vendor/codemirror/
// codemirror.min.js as an IIFE exposing the named exports on `window.BlxCM`.
//
// The Rust glue (`src/workbench/file_preview/codemirror_glue.rs`) only ever
// calls the small wrapper API below (create / setDoc / getDoc / destroy /
// selectionLines), so the CodeMirror module graph stays an implementation
// detail of this file.

import { EditorView, basicSetup } from "codemirror";
import { EditorState } from "@codemirror/state";
import { keymap } from "@codemirror/view";
import { indentWithTab } from "@codemirror/commands";
import { StreamLanguage } from "@codemirror/language";
import { oneDark } from "@codemirror/theme-one-dark";

// Native CodeMirror 6 language packages (richest support).
import { rust } from "@codemirror/lang-rust";
import { javascript } from "@codemirror/lang-javascript";
import { python } from "@codemirror/lang-python";
import { json } from "@codemirror/lang-json";
import { css } from "@codemirror/lang-css";
import { html } from "@codemirror/lang-html";
import { markdown } from "@codemirror/lang-markdown";
import { xml } from "@codemirror/lang-xml";
import { cpp } from "@codemirror/lang-cpp";
import { java } from "@codemirror/lang-java";
import { php } from "@codemirror/lang-php";
import { sql } from "@codemirror/lang-sql";
import { yaml } from "@codemirror/lang-yaml";
import { go } from "@codemirror/lang-go";

// Legacy (CM5-ported) stream modes — broaden coverage to match the highlight.js
// viewer for languages without a dedicated CM6 package.
import { shell } from "@codemirror/legacy-modes/mode/shell";
import { toml } from "@codemirror/legacy-modes/mode/toml";
import { properties } from "@codemirror/legacy-modes/mode/properties";
import { ruby } from "@codemirror/legacy-modes/mode/ruby";
import { lua } from "@codemirror/legacy-modes/mode/lua";
import { perl } from "@codemirror/legacy-modes/mode/perl";
import { r } from "@codemirror/legacy-modes/mode/r";
import { clojure } from "@codemirror/legacy-modes/mode/clojure";
import { erlang } from "@codemirror/legacy-modes/mode/erlang";
import { haskell } from "@codemirror/legacy-modes/mode/haskell";
import { swift } from "@codemirror/legacy-modes/mode/swift";
import { dockerFile } from "@codemirror/legacy-modes/mode/dockerfile";
import { diff } from "@codemirror/legacy-modes/mode/diff";
import { cmake } from "@codemirror/legacy-modes/mode/cmake";
import { powerShell } from "@codemirror/legacy-modes/mode/powershell";
import { julia } from "@codemirror/legacy-modes/mode/julia";
import { groovy } from "@codemirror/legacy-modes/mode/groovy";
import { vb } from "@codemirror/legacy-modes/mode/vb";
import { protobuf } from "@codemirror/legacy-modes/mode/protobuf";
import { csharp, kotlin, scala, objectiveC, dart } from "@codemirror/legacy-modes/mode/clike";
import { oCaml, fSharp } from "@codemirror/legacy-modes/mode/mllike";

// `legacy(mode)` adapts a CM5 stream mode into a CM6 language extension.
const legacy = (mode) => () => StreamLanguage.define(mode);

// Map a language name (the alias resolved Rust-side) to a CodeMirror language
// extension. Unknown names fall back to plain text (no language extension).
const LANGS = {
  rust: () => rust(),
  javascript: () => javascript(),
  jsx: () => javascript({ jsx: true }),
  typescript: () => javascript({ typescript: true }),
  tsx: () => javascript({ jsx: true, typescript: true }),
  python: () => python(),
  json: () => json(),
  css: () => css(),
  html: () => html(),
  markdown: () => markdown(),
  xml: () => xml(),
  cpp: () => cpp(),
  c: () => cpp(),
  java: () => java(),
  php: () => php(),
  sql: () => sql(),
  yaml: () => yaml(),
  go: () => go(),
  // Legacy stream modes:
  shell: legacy(shell),
  toml: legacy(toml),
  properties: legacy(properties),
  ruby: legacy(ruby),
  lua: legacy(lua),
  perl: legacy(perl),
  r: legacy(r),
  clojure: legacy(clojure),
  erlang: legacy(erlang),
  haskell: legacy(haskell),
  swift: legacy(swift),
  dockerfile: legacy(dockerFile),
  diff: legacy(diff),
  cmake: legacy(cmake),
  powershell: legacy(powerShell),
  julia: legacy(julia),
  groovy: legacy(groovy),
  vb: legacy(vb),
  protobuf: legacy(protobuf),
  csharp: legacy(csharp),
  kotlin: legacy(kotlin),
  scala: legacy(scala),
  objc: legacy(objectiveC),
  dart: legacy(dart),
  ocaml: legacy(oCaml),
  fsharp: legacy(fSharp),
};

function langExt(name) {
  const make = name && LANGS[name];
  if (!make) return [];
  try {
    return make();
  } catch (_e) {
    return [];
  }
}

// Editor chrome themed with the app's CSS custom properties so the editor
// follows the active BLXCode theme. Syntax token colors come from oneDark
// (applied first; this override wins for chrome). Background is transparent so
// the panel background shows through, matching the read-only view.
const blxChrome = EditorView.theme(
  {
    "&": {
      color: "var(--text)",
      backgroundColor: "transparent",
      height: "100%",
      fontSize: "0.78rem",
    },
    ".cm-scroller": {
      fontFamily: "var(--font-mono)",
      lineHeight: "1.25rem",
      overflow: "auto",
    },
    ".cm-content": { caretColor: "var(--text)" },
    ".cm-gutters": {
      backgroundColor: "color-mix(in srgb, var(--surface) 60%, transparent)",
      color: "var(--text-muted)",
      border: "none",
      borderRight: "1px solid color-mix(in srgb, var(--border) 60%, transparent)",
    },
    ".cm-activeLine": {
      backgroundColor: "color-mix(in srgb, var(--text) 5%, transparent)",
    },
    ".cm-activeLineGutter": { backgroundColor: "transparent" },
    ".cm-foldGutter span": { color: "var(--text-muted)" },
    "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection":
      { backgroundColor: "color-mix(in srgb, var(--accent) 32%, transparent)" },
    ".cm-cursor, .cm-dropCursor": { borderLeftColor: "var(--text)" },
    "&.cm-focused .cm-matchingBracket": {
      backgroundColor: "color-mix(in srgb, var(--accent) 22%, transparent)",
      outline: "none",
    },
  },
  { dark: true },
);

/**
 * Create an editor inside `parent`.
 * opts: { doc, language, onChange(str), onSave(), readOnly }
 * Returns the EditorView (opaque handle for the other helpers).
 */
export function create(parent, opts) {
  const o = opts || {};
  let syncing = false;

  const updateListener = EditorView.updateListener.of((u) => {
    if (u.docChanged && !syncing && typeof o.onChange === "function") {
      o.onChange(u.state.doc.toString());
    }
  });

  const saveKeymap = keymap.of([
    {
      key: "Mod-s",
      preventDefault: true,
      run: () => {
        if (typeof o.onSave === "function") o.onSave();
        return true;
      },
    },
    indentWithTab,
  ]);

  const extensions = [
    basicSetup,
    saveKeymap,
    langExt(o.language),
    oneDark,
    blxChrome,
    updateListener,
  ];
  if (o.readOnly) {
    extensions.push(EditorState.readOnly.of(true));
  }

  const view = new EditorView({
    parent,
    state: EditorState.create({ doc: o.doc || "", extensions }),
  });

  // External updates (revert / reload) must not re-fire onChange.
  view.__blxSetDoc = (text) => {
    if (text === view.state.doc.toString()) return;
    syncing = true;
    try {
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: text },
      });
    } finally {
      syncing = false;
    }
  };
  return view;
}

export function setDoc(view, text) {
  if (view && view.__blxSetDoc) view.__blxSetDoc(text);
}

export function getDoc(view) {
  return view ? view.state.doc.toString() : "";
}

export function destroy(view) {
  if (view) view.destroy();
}

/**
 * 1-based [from, to] line numbers covered by the primary selection. A collapsed
 * caret yields a single line (from === to). Used for the right-click handoff
 * menu so it captures the caret line or the whole selection.
 */
export function selectionLines(view) {
  if (!view) return [1, 1];
  const sel = view.state.selection.main;
  const from = view.state.doc.lineAt(sel.from).number;
  // For a non-empty selection that ends exactly at a line start, don't pull in
  // the following line.
  const toPos = sel.to > sel.from ? sel.to - 1 : sel.to;
  const to = view.state.doc.lineAt(toPos).number;
  return [Math.min(from, to), Math.max(from, to)];
}

// Re-export a couple of primitives in case future glue needs them.
export { EditorView, EditorState };
