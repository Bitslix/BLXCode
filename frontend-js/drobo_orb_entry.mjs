import * as THREE from "three";
import { GLTFLoader } from "three/examples/jsm/loaders/GLTFLoader.js";

const MODEL_URL = "/public/assets/Drobo.glb";
const instances = new Map();
let nextId = 1;

const loader = new GLTFLoader();
const prefersReducedMotion = window.matchMedia?.("(prefers-reduced-motion: reduce)");
const BASE_ROTATION = {
  x: -0.05,
  y: -Math.PI / 2 + 0.08,
  z: -0.02,
};
const MODEL_Y_OFFSET = 0.34;

function clamp(value, min = -1, max = 1) {
  return Math.max(min, Math.min(max, value));
}

function trackingRectFor(container) {
  const root =
    container.closest?.(".workbench-right") ||
    container.closest?.(".workbench-right-slot") ||
    container.closest?.(".workbench-agent-pane") ||
    container;
  return root.getBoundingClientRect();
}

function readCssVar(name, fallback = "") {
  try {
    const v = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
    return v || fallback;
  } catch (_) {
    return fallback;
  }
}

function cssColor(name, fallback) {
  const raw = readCssVar(name, fallback);
  try {
    return new THREE.Color(raw);
  } catch (_) {
    const rgba = raw.match(/rgba?\(([^)]+)\)/i);
    if (rgba) {
      const parts = rgba[1].split(",").map((part) => Number.parseFloat(part.trim()));
      if (parts.length >= 3 && parts.every((part) => Number.isFinite(part))) {
        return new THREE.Color(parts[0] / 255, parts[1] / 255, parts[2] / 255);
      }
    }
    return new THREE.Color(fallback);
  }
}

function materialKind(material) {
  switch (material?.name) {
    case "mat8":
      return "screen";
    case "mat14":
      return "screen-accent";
    case "mat15":
      return "chassis";
    case "mat21":
      return "body";
    case "mat22":
      return "joint";
    case "mat23":
      return "dark";
    default:
      return "body";
  }
}

function applyTheme(rec) {
  if (!rec?.materials) return;
  const palette = {
    body: cssColor("--text", "#e7e8ef").lerp(cssColor("--bg-panel", "#242633"), 0.18),
    chassis: cssColor("--text-muted", "#9aa3b8").lerp(cssColor("--border-strong", "#4f5668"), 0.28),
    joint: cssColor("--border-strong", "#4f5668"),
    dark: cssColor("--bg-app", "#11131a").lerp(new THREE.Color("#000000"), 0.52),
    screen: cssColor("--accent", "#bd93f9"),
    screenAccent: cssColor("--accent-cool", "#7dcfff"),
  };

  for (const material of rec.materials) {
    const kind = material.userData.droboKind;
    const color =
      kind === "screen"
        ? palette.screen
        : kind === "screen-accent"
          ? palette.screenAccent
          : kind === "chassis"
            ? palette.chassis
            : kind === "joint"
              ? palette.joint
              : kind === "dark"
                ? palette.dark
                : palette.body;
    material.color.copy(color);
    material.emissive.copy(kind === "screen" || kind === "screen-accent" ? color : palette.dark);
    material.emissiveIntensity =
      kind === "screen" ? 0.32 : kind === "screen-accent" ? 0.46 : kind === "dark" ? 0.03 : 0.015;
    material.needsUpdate = true;
  }

  rec.keyLight.color.copy(cssColor("--accent-cool", "#7dcfff"));
  rec.fillLight.color.copy(cssColor("--accent", "#bd93f9"));
  rec.rimLight.color.copy(palette.body);
}

function fitModelToGroup(model, group) {
  const box = new THREE.Box3().setFromObject(model);
  const size = new THREE.Vector3();
  const center = new THREE.Vector3();
  box.getSize(size);
  box.getCenter(center);
  const maxSide = Math.max(size.x, size.y, size.z) || 1;
  model.position.sub(center);
  model.scale.setScalar(2.28 / maxSide);
  group.add(model);
}

function cloneAndPrepareMaterials(model) {
  const materials = [];
  model.traverse((obj) => {
    if (!obj.isMesh) return;
    obj.castShadow = false;
    obj.receiveShadow = false;
    const src = Array.isArray(obj.material) ? obj.material : [obj.material];
    const cloned = src.map((material) => {
      const next = material.clone();
      next.userData.droboKind = materialKind(material);
      next.metalness = 0.04;
      next.roughness = 0.78;
      materials.push(next);
      return next;
    });
    obj.material = Array.isArray(obj.material) ? cloned : cloned[0];
  });
  return materials;
}

function create(container) {
  const id = nextId++;
  container.classList.remove("drobo-orb__stage--loaded", "drobo-orb__stage--failed");

  let renderer;
  try {
    renderer = new THREE.WebGLRenderer({
      alpha: true,
      antialias: true,
      preserveDrawingBuffer: true,
      powerPreference: "low-power",
    });
  } catch (_) {
    container.classList.add("drobo-orb__stage--failed");
    instances.set(id, { id, container, failed: true });
    return id;
  }
  renderer.setClearColor(0x000000, 0);
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  renderer.domElement.className = "drobo-orb__canvas";
  renderer.domElement.setAttribute("aria-hidden", "true");
  container.appendChild(renderer.domElement);

  const scene = new THREE.Scene();
  const camera = new THREE.PerspectiveCamera(28, 1, 0.1, 100);
  camera.position.set(0, 0.12, 6.9);
  camera.lookAt(0, 0, 0);

  const group = new THREE.Group();
  group.rotation.set(BASE_ROTATION.x, BASE_ROTATION.y, BASE_ROTATION.z);
  scene.add(group);

  const ambient = new THREE.HemisphereLight(0xffffff, 0x0d1018, 1.65);
  scene.add(ambient);
  const keyLight = new THREE.DirectionalLight(0x7dcfff, 1.65);
  keyLight.position.set(2.2, 2.4, 3.4);
  scene.add(keyLight);
  const fillLight = new THREE.DirectionalLight(0xbd93f9, 0.74);
  fillLight.position.set(-2.6, 1.3, 2.2);
  scene.add(fillLight);
  const rimLight = new THREE.DirectionalLight(0xffffff, 0.56);
  rimLight.position.set(0, 2.5, -3.2);
  scene.add(rimLight);

  const rec = {
    id,
    container,
    renderer,
    scene,
    camera,
    group,
    keyLight,
    fillLight,
    rimLight,
    materials: [],
    resizeObserver: null,
    frame: 0,
    createdAt: performance.now(),
    pointer: { x: 0, y: 0 },
    target: { x: 0, y: 0 },
    rotation: { x: BASE_ROTATION.x, y: BASE_ROTATION.y, z: BASE_ROTATION.z },
    state: { active: false, transcribing: false, compact: false },
    reducedMotion: Boolean(prefersReducedMotion?.matches),
  };
  instances.set(id, rec);

  const onPointerMove = (event) => {
    const trackRect = trackingRectFor(container);
    if (
      event.clientX < trackRect.left ||
      event.clientX > trackRect.right ||
      event.clientY < trackRect.top ||
      event.clientY > trackRect.bottom
    ) {
      onPointerLeave();
      return;
    }

    const orbRect = container.getBoundingClientRect();
    const centerX = orbRect.left + orbRect.width / 2;
    const centerY = orbRect.top + orbRect.height / 2;
    const reachX = Math.max(orbRect.width * 1.45, trackRect.width * 0.42);
    const reachY = Math.max(orbRect.height * 1.45, trackRect.height * 0.36);
    rec.pointer.x = clamp((event.clientX - centerX) / reachX);
    rec.pointer.y = clamp((event.clientY - centerY) / reachY);
  };
  const onPointerLeave = () => {
    rec.pointer.x = 0;
    rec.pointer.y = 0;
  };
  const onPointerOut = (event) => {
    if (!event.relatedTarget) onPointerLeave();
  };
  rec.onPointerMove = onPointerMove;
  rec.onPointerLeave = onPointerLeave;
  rec.onPointerOut = onPointerOut;
  window.addEventListener("pointermove", onPointerMove);
  window.addEventListener("blur", onPointerLeave);
  window.addEventListener("pointerout", onPointerOut);

  rec.resizeObserver = new ResizeObserver(() => resize(id));
  rec.resizeObserver.observe(container);

  loader.load(
    MODEL_URL,
    (gltf) => {
      if (!instances.has(id)) return;
      rec.materials = cloneAndPrepareMaterials(gltf.scene);
      fitModelToGroup(gltf.scene, group);
      applyTheme(rec);
      container.classList.add("drobo-orb__stage--loaded");
      resize(id);
    },
    undefined,
    () => {
      container.classList.add("drobo-orb__stage--failed");
    },
  );

  resize(id);
  animate(id);
  return id;
}

function animate(id) {
  const rec = instances.get(id);
  if (!rec) return;
  const tick = () => {
    if (!instances.has(id)) return;
    const now = performance.now();
    const activeBoost = rec.state.active ? 1.0 : 0.0;
    const pulseBoost = rec.state.transcribing ? 1.0 : 0.0;
    const compact = rec.state.compact || rec.container.clientWidth < 60;
    const idleScale = rec.reducedMotion ? 0 : compact ? 0.055 : 0.09;
    const cursorScale = rec.reducedMotion ? 0.11 : compact ? 0.22 : 0.34;
    const idle = (now - rec.createdAt) / 1000;

    rec.target.x = BASE_ROTATION.x + rec.pointer.y * cursorScale + Math.sin(idle * 1.4) * idleScale;
    rec.target.y = BASE_ROTATION.y + rec.pointer.x * cursorScale + Math.sin(idle * 0.8) * idleScale;
    rec.target.z = BASE_ROTATION.z + rec.pointer.x * 0.06 + Math.sin(idle * 1.1) * idleScale * 0.28;

    const damp = rec.reducedMotion ? 0.1 : 0.075;
    rec.rotation.x += (rec.target.x - rec.rotation.x) * damp;
    rec.rotation.y += (rec.target.y - rec.rotation.y) * damp;
    rec.rotation.z += (rec.target.z - rec.rotation.z) * damp;
    rec.group.rotation.set(rec.rotation.x, rec.rotation.y, rec.rotation.z);

    const bob = rec.reducedMotion ? 0 : Math.sin(idle * (rec.state.active ? 3.2 : 1.7)) * (compact ? 0.012 : 0.026);
    rec.group.position.y = MODEL_Y_OFFSET + bob;
    rec.group.scale.setScalar(1 + activeBoost * 0.045 + pulseBoost * (0.025 + Math.sin(idle * 8) * 0.012));

    rec.renderer.render(rec.scene, rec.camera);
    rec.frame = requestAnimationFrame(tick);
  };
  rec.frame = requestAnimationFrame(tick);
}

function setState(id, state = {}) {
  const rec = instances.get(id);
  if (!rec) return false;
  if (rec.failed) return true;
  rec.state.active = Boolean(state.active);
  rec.state.transcribing = Boolean(state.transcribing);
  rec.state.compact = Boolean(state.compact);
  return true;
}

function resize(id) {
  const rec = instances.get(id);
  if (!rec) return false;
  if (rec.failed) return true;
  const rect = rec.container.getBoundingClientRect();
  const width = Math.max(1, Math.floor(rect.width || rec.container.clientWidth || 1));
  const height = Math.max(1, Math.floor(rect.height || rec.container.clientHeight || 1));
  rec.renderer.setSize(width, height, false);
  rec.camera.aspect = width / height;
  rec.camera.position.z = width < 64 ? 7.1 : 6.9;
  rec.camera.updateProjectionMatrix();
  return true;
}

function dispose(id) {
  const rec = instances.get(id);
  if (!rec) return;
  instances.delete(id);
  if (rec.failed) return;
  if (rec.frame) cancelAnimationFrame(rec.frame);
  rec.resizeObserver?.disconnect();
  window.removeEventListener("pointermove", rec.onPointerMove);
  window.removeEventListener("blur", rec.onPointerLeave);
  window.removeEventListener("pointerout", rec.onPointerOut);
  rec.scene.traverse((obj) => {
    if (!obj.isMesh) return;
    obj.geometry?.dispose?.();
    const materials = Array.isArray(obj.material) ? obj.material : [obj.material];
    for (const material of materials) material?.dispose?.();
  });
  rec.renderer.dispose();
  rec.renderer.domElement.remove();
}

window.addEventListener("blxcode-theme-changed", () => {
  for (const rec of instances.values()) applyTheme(rec);
});

prefersReducedMotion?.addEventListener?.("change", (event) => {
  for (const rec of instances.values()) rec.reducedMotion = Boolean(event.matches);
});

window.__blxcodeDroboOrb = {
  create,
  setState,
  resize,
  dispose,
};

window.dispatchEvent(new CustomEvent("blxcode-drobo-orb-api-ready"));
