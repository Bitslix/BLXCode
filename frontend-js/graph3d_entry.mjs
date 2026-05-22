import ForceGraph3D from "3d-force-graph";
import * as THREE from "three";
import SpriteText from "three-spritetext";

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

function applyGraph3dTheme(rec) {
  if (!rec?.graph) return;
  rec.graph.backgroundColor("rgba(0,0,0,0)");
  rec.graph.linkColor(() => readCssVar("--overlay-4", "rgba(255,255,255,0.14)"));
  rec.graph.nodeThreeObject(makeNodeObject);
  rec.graph.refresh();
}

window.addEventListener("blxcode-theme-changed", () => {
  for (const rec of instances.values()) {
    applyGraph3dTheme(rec);
  }
});

function cleanLabel(raw) {
  const tail = String(raw || "")
    .replace(/\\/g, "/")
    .split("/")
    .filter(Boolean)
    .pop() || "";
  const withoutExt = tail.replace(/\.[a-z0-9]+$/i, "");
  const words = withoutExt
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .split(/[-_\s.]+/)
    .map((word) => word.trim())
    .filter(Boolean);
  const acronyms = new Set(["api", "ui", "ux", "url", "http", "https", "json", "css", "html", "js", "ts", "2d", "3d"]);
  const label = words
    .map((word) => {
      const lower = word.toLowerCase();
      if (acronyms.has(lower)) return lower.toUpperCase();
      return lower.charAt(0).toUpperCase() + lower.slice(1);
    })
    .join(" ");
  return label || String(raw || "");
}

function wrapLabel(label) {
  const words = String(label).split(/\s+/).filter(Boolean);
  const lines = [];
  let line = "";
  for (const word of words) {
    const next = line ? `${line} ${word}` : word;
    if (next.length > 24 && line) {
      lines.push(line);
      line = word;
    } else {
      line = next;
    }
  }
  if (line) lines.push(line);
  return lines.slice(0, 2).join("\n");
}

function normalizeGraphData(graphData) {
  const nodes = Array.isArray(graphData?.nodes) ? graphData.nodes : [];
  const links = Array.isArray(graphData?.edges)
    ? graphData.edges.map((edge) => ({
        source: edge.source,
        target: edge.target,
      }))
    : [];
  return {
    nodes: nodes.map((node) => ({
      id: String(node.id),
      label: cleanLabel(node.label || node.id),
      tags: Array.isArray(node.tags) ? node.tags : [],
      orphan: Boolean(node.orphan),
      color: typeof node.color === "string" && node.color.trim() ? node.color : null,
      category: typeof node.category === "string" && node.category.trim() ? node.category : null,
    })),
    links,
  };
}

function colorForNode(node) {
  if (node.color) return node.color;
  if (node.orphan) return "#9aa3b8";
  const tag = node.tags?.[0] || "";
  let hash = 0;
  for (let i = 0; i < tag.length; i += 1) {
    hash = (hash * 31 + tag.charCodeAt(i)) >>> 0;
  }
  const palette = ["#7dd3fc", "#86efac", "#f9a8d4", "#fde68a", "#c4b5fd", "#fca5a5"];
  return palette[hash % palette.length];
}

function makeNodeObject(node) {
  const color = colorForNode(node);
  const group = new THREE.Group();
  const geometry = new THREE.SphereGeometry(5.8, 24, 24);
  const material = new THREE.MeshPhongMaterial({
    color,
    emissive: color,
    emissiveIntensity: 0.22,
    shininess: 70,
  });
  const sphere = new THREE.Mesh(geometry, material);
  const halo = new THREE.Mesh(
    new THREE.SphereGeometry(8.5, 24, 24),
    new THREE.MeshBasicMaterial({
      color,
      transparent: true,
      opacity: 0.12,
      depthWrite: false,
    }),
  );
  const label = new SpriteText(wrapLabel(node.label || node.id));
  label.color = readCssVar("--text", "rgba(238,239,245,0.92)");
  label.backgroundColor = readCssVar("--scrim-bg", "rgba(8,10,16,0.58)");
  label.borderColor = readCssVar("--overlay-4", "rgba(255,255,255,0.14)");
  label.borderWidth = 0.25;
  label.borderRadius = 2;
  label.padding = 2.6;
  label.textHeight = 4.2;
  label.center.set(0, 0.5);
  label.position.set(12.5, 0, 0);
  label.material.depthWrite = false;
  label.material.depthTest = false;
  label.renderOrder = 10;
  group.add(halo);
  group.add(sphere);
  group.add(label);
  return group;
}

function nowMs() {
  return performance.now();
}

function linkWobble(link) {
  if (link._dragCurve != null) return link._dragCurve;
  const spring = link._spring;
  if (!spring) return 0;
  const t = (nowMs() - spring.started) / 1000;
  const duration = 1.05;
  if (t >= duration) {
    link._spring = null;
    return 0;
  }
  const decay = Math.pow(1 - t / duration, 2.1);
  return Math.sin(t * 9.5 + spring.phase) * spring.amplitude * decay;
}

function linkRotation(link) {
  if (link._dragRotation != null) return link._dragRotation;
  return link._spring?.rotation || 0;
}

function pluckLink(rec, link, amplitude = 0.1, rotation = Math.random() * Math.PI) {
  if (!link) return;
  link._spring = {
    started: nowMs(),
    amplitude,
    phase: Math.random() * Math.PI * 2,
    rotation,
  };
  startWobbleLoop(rec);
}

function pluckIncidentLinks(rec, node, amplitude = 0.06) {
  const nodeId = String(node?.id || "");
  if (!nodeId) return;
  for (const link of rec.links) {
    const source = typeof link.source === "object" ? link.source.id : link.source;
    const target = typeof link.target === "object" ? link.target.id : link.target;
    if (String(source) === nodeId || String(target) === nodeId) {
      pluckLink(rec, link, amplitude);
    }
  }
}

function refreshLinks(rec) {
  rec.graph.linkCurvature((link) => linkWobble(link));
  rec.graph.linkCurveRotation((link) => linkRotation(link));
  rec.graph.refresh?.();
}

function startWobbleLoop(rec) {
  if (rec.wobbleFrame) return;
  const tick = () => {
    rec.wobbleFrame = 0;
    let active = Boolean(rec.draggedLink);
    for (const link of rec.links) {
      if (link._spring || link._dragCurve != null) {
        active = true;
        break;
      }
    }
    refreshLinks(rec);
    if (active) {
      rec.wobbleFrame = requestAnimationFrame(tick);
    }
  };
  rec.wobbleFrame = requestAnimationFrame(tick);
}

function installLinkDrag(rec) {
  const onPointerDown = (event) => {
    if (!rec.hoveredLink) return;
    event.preventDefault();
    rec.draggedLink = rec.hoveredLink;
    rec.dragStart = { x: event.clientX, y: event.clientY };
    rec.draggedLink._spring = null;
    rec.draggedLink._dragCurve = 0.001;
    rec.draggedLink._dragRotation = 0;
    try {
      rec.graph.controls().enabled = false;
    } catch (_) {}
    startWobbleLoop(rec);
  };
  const onPointerMove = (event) => {
    if (!rec.draggedLink || !rec.dragStart) return;
    const dx = event.clientX - rec.dragStart.x;
    const dy = event.clientY - rec.dragStart.y;
    const pull = Math.min(0.22, Math.hypot(dx, dy) / 220);
    rec.draggedLink._dragCurve = pull * (dy < 0 ? -1 : 1);
    rec.draggedLink._dragRotation = Math.atan2(dy, dx);
    refreshLinks(rec);
  };
  const onPointerUp = (event) => {
    if (!rec.draggedLink) return;
    const link = rec.draggedLink;
    const amplitude = Math.max(0.045, Math.min(0.14, Math.abs(link._dragCurve || 0)));
    const rotation = link._dragRotation || 0;
    link._dragCurve = null;
    link._dragRotation = null;
    rec.draggedLink = null;
    rec.dragStart = null;
    try {
      rec.graph.controls().enabled = true;
    } catch (_) {}
    pluckLink(rec, link, amplitude, rotation || Math.atan2(event.movementY || 1, event.movementX || 1));
  };
  rec.container.addEventListener("pointerdown", onPointerDown);
  window.addEventListener("pointermove", onPointerMove);
  window.addEventListener("pointerup", onPointerUp);
  return () => {
    rec.container.removeEventListener("pointerdown", onPointerDown);
    window.removeEventListener("pointermove", onPointerMove);
    window.removeEventListener("pointerup", onPointerUp);
  };
}

function replayPendingFlyTo(id, rec) {
  if (!rec.layoutReady || rec.pendingFlyTo.length === 0) return;
  const pending = rec.pendingFlyTo.splice(0);
  for (const item of pending) {
    flyToNode(id, item.nodeId, item.ms);
  }
}

function flyToNode(id, nodeId, ms = 900) {
  const rec = instances.get(id);
  if (!rec) return false;
  if (!rec.layoutReady) {
    rec.pendingFlyTo.push({ nodeId, ms });
    return true;
  }
  const node = rec.nodesById.get(String(nodeId));
  if (!node) return false;
  const distance = 96;
  const dist = Math.hypot(node.x || 0, node.y || 0, node.z || 0) || 1;
  const distRatio = 1 + distance / dist;
  rec.graph.cameraPosition(
    {
      x: (node.x || 0) * distRatio,
      y: (node.y || 0) * distRatio,
      z: (node.z || 0) * distRatio,
    },
    node,
    ms,
  );
  return true;
}

function resize(id) {
  const rec = instances.get(id);
  if (!rec) return;
  const rect = rec.container.getBoundingClientRect();
  if (rect.width <= 1 || rect.height <= 1) return;
  rec.graph.width(rect.width).height(rect.height);
  applyResponsiveForces(rec);
  if (rec.layoutReady) {
    rec.graph.zoomToFit(320, fitPadding(rec));
  }
}

function fitPadding(rec) {
  const rect = rec.container.getBoundingClientRect();
  const minSide = Math.min(rect.width || 480, rect.height || 360);
  return Math.max(44, Math.min(92, minSide * 0.11));
}

function applyResponsiveForces(rec) {
  const rect = rec.container.getBoundingClientRect();
  const minSide = Math.max(280, Math.min(rect.width || 480, rect.height || 360));
  const nodeCount = Math.max(1, rec.nodesById.size || 1);
  const densityFactor = Math.max(0.62, Math.min(1, 8 / Math.max(8, nodeCount)));
  const distance = Math.max(58, Math.min(116, minSide * 0.12 * densityFactor));
  const charge = -Math.max(120, Math.min(300, distance * 2.15 + nodeCount * 8));
  try {
    rec.graph.d3Force("link").distance(distance).strength(0.34);
    rec.graph.d3Force("charge").strength(charge).distanceMax(distance * 4.1);
    rec.graph.d3VelocityDecay(0.34);
    rec.graph.d3Force("category", categoryClusterForce(rec));
  } catch (_) {}
}

function categoryClusterForce(rec) {
  // Pulls each node toward the centroid of its category, so nodes from the
  // same memory category cluster together. Strength scales with simulation
  // alpha so it fades as the layout settles.
  return (alpha) => {
    const data = rec.graph.graphData();
    const nodes = data?.nodes || [];
    if (!nodes.length) return;
    const centroids = new Map();
    for (const node of nodes) {
      const key = node.category;
      if (!key) continue;
      const entry = centroids.get(key) || { x: 0, y: 0, z: 0, count: 0 };
      entry.x += node.x || 0;
      entry.y += node.y || 0;
      entry.z += node.z || 0;
      entry.count += 1;
      centroids.set(key, entry);
    }
    for (const entry of centroids.values()) {
      if (entry.count > 0) {
        entry.x /= entry.count;
        entry.y /= entry.count;
        entry.z /= entry.count;
      }
    }
    const strength = 0.08 * alpha;
    for (const node of nodes) {
      const entry = node.category ? centroids.get(node.category) : null;
      if (!entry || entry.count < 2) continue;
      node.vx = (node.vx || 0) + (entry.x - (node.x || 0)) * strength;
      node.vy = (node.vy || 0) + (entry.y - (node.y || 0)) * strength;
      node.vz = (node.vz || 0) + (entry.z - (node.z || 0)) * strength;
    }
  };
}

window.__blxcodeGraph3d = {
  create(container) {
    const id = nextId++;
    const graph = ForceGraph3D()(container)
      .backgroundColor("rgba(0,0,0,0)")
      .nodeId("id")
      .nodeLabel("label")
      .nodeThreeObject(makeNodeObject)
      .linkColor(() => readCssVar("--overlay-4", "rgba(255,255,255,0.14)"))
      .linkOpacity(0.34)
      .linkWidth(1)
      .linkCurvature((link) => linkWobble(link))
      .linkCurveRotation((link) => linkRotation(link))
      .onLinkHover((link) => {
        const rec = instances.get(id);
        if (!rec || rec.draggedLink) return;
        rec.hoveredLink = link || null;
        container.style.cursor = link ? "grab" : "";
      })
      .onLinkClick((link) => {
        const rec = instances.get(id);
        if (rec) pluckLink(rec, link, 0.11);
      })
      .onNodeClick((node) => {
        window.dispatchEvent(
          new CustomEvent("blxcode-graph3d-node-click", {
            detail: { graphId: id, nodeId: String(node.id) },
          }),
        );
      })
      .onNodeDrag((node) => {
        const rec = instances.get(id);
        if (rec) pluckIncidentLinks(rec, node, 0.035);
      })
      .onNodeDragEnd((node) => {
        const rec = instances.get(id);
        if (rec) pluckIncidentLinks(rec, node, 0.1);
      })
      .onEngineStop(() => {
        const rec = instances.get(id);
        if (!rec) return;
        rec.layoutReady = true;
        if (rec.autoFitPending && rec.pendingFlyTo.length === 0) {
          rec.graph.zoomToFit(520, fitPadding(rec));
        }
        rec.autoFitPending = false;
        replayPendingFlyTo(id, rec);
      });

    const rec = {
      graph,
      container,
      nodesById: new Map(),
      layoutReady: false,
      pendingFlyTo: [],
      resizeObserver: null,
      links: [],
      hoveredLink: null,
      draggedLink: null,
      dragStart: null,
      wobbleFrame: 0,
      removeLinkDrag: null,
      autoFitPending: true,
    };
    applyResponsiveForces(rec);
    rec.removeLinkDrag = installLinkDrag(rec);
    rec.resizeObserver = new ResizeObserver(() => resize(id));
    rec.resizeObserver.observe(container);
    instances.set(id, rec);
    requestAnimationFrame(() => resize(id));
    return id;
  },
  dispose(id) {
    const rec = instances.get(id);
    if (!rec) return;
    try {
      rec.resizeObserver?.disconnect();
    } catch (_) {}
    try {
      rec.removeLinkDrag?.();
    } catch (_) {}
    if (rec.wobbleFrame) cancelAnimationFrame(rec.wobbleFrame);
    try {
      rec.graph._destructor?.();
    } catch (_) {}
    instances.delete(id);
  },
  setData(id, graphData) {
    const rec = instances.get(id);
    if (!rec) return false;
    const data = normalizeGraphData(graphData);
    rec.layoutReady = false;
    rec.nodesById = new Map(data.nodes.map((node) => [node.id, node]));
    rec.links = data.links;
    rec.hoveredLink = null;
    rec.draggedLink = null;
    rec.autoFitPending = true;
    applyResponsiveForces(rec);
    rec.graph.graphData(data);
    resize(id);
    return true;
  },
  zoom(id, factor) {
    const rec = instances.get(id);
    if (!rec) return false;
    const camera = rec.graph.camera();
    const k = Number(factor) > 0 ? Number(factor) : 1;
    rec.graph.cameraPosition(
      {
        x: camera.position.x / k,
        y: camera.position.y / k,
        z: camera.position.z / k,
      },
      undefined,
      240,
    );
    return true;
  },
  resetView(id) {
    const rec = instances.get(id);
    if (!rec) return false;
    rec.graph.zoomToFit(500, 48);
    return true;
  },
  flyToNode,
  resize,
};

window.dispatchEvent(new CustomEvent("blxcode-graph3d-api-ready"));
