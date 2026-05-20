import ForceGraph3D from "3d-force-graph";
import * as THREE from "three";
import SpriteText from "three-spritetext";

const instances = new Map();
let nextId = 1;

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
      label: String(node.label || node.id),
      tags: Array.isArray(node.tags) ? node.tags : [],
      orphan: Boolean(node.orphan),
    })),
    links,
  };
}

function colorForNode(node) {
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
  const label = new SpriteText(node.label || node.id);
  label.color = "rgba(238,239,245,0.92)";
  label.backgroundColor = "rgba(8,10,16,0.58)";
  label.borderColor = "rgba(255,255,255,0.14)";
  label.borderWidth = 0.25;
  label.borderRadius = 2;
  label.padding = 2.6;
  label.textHeight = 4.2;
  label.position.set(11.5, 0.5, 0);
  label.material.depthWrite = false;
  label.renderOrder = 10;
  group.add(halo);
  group.add(sphere);
  group.add(label);
  return group;
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
}

window.__blxcodeGraph3d = {
  create(container) {
    const id = nextId++;
    const graph = ForceGraph3D()(container)
      .backgroundColor("rgba(0,0,0,0)")
      .nodeId("id")
      .nodeLabel("label")
      .nodeThreeObject(makeNodeObject)
      .linkColor(() => "rgba(190,205,235,0.28)")
      .linkOpacity(0.34)
      .linkWidth(1)
      .onNodeClick((node) => {
        window.dispatchEvent(
          new CustomEvent("blxcode-graph3d-node-click", {
            detail: { graphId: id, nodeId: String(node.id) },
          }),
        );
      })
      .onEngineStop(() => {
        const rec = instances.get(id);
        if (!rec) return;
        rec.layoutReady = true;
        replayPendingFlyTo(id, rec);
      });

    const rec = {
      graph,
      container,
      nodesById: new Map(),
      layoutReady: false,
      pendingFlyTo: [],
      resizeObserver: null,
    };
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
