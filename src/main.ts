import "./style.css";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import * as THREE from "three";
import { GLTFLoader, type GLTF } from "three/examples/jsm/loaders/GLTFLoader.js";
import { RoomEnvironment } from "three/examples/jsm/environments/RoomEnvironment.js";
import { CatBinAudio } from "./cat-bin-audio";
import { BinMotion } from "./bin-motion";

type BinSettings = { confirmDelete: boolean; soundEnabled: boolean; scalePercent: number; modelName?: string };
type BinResult = { ok: boolean; error?: string; cancelled?: boolean };

const native = "__TAURI_INTERNALS__" in window;
const button = document.querySelector<HTMLButtonElement>("#trash-bin")!;
const canvas = document.querySelector<HTMLCanvasElement>("#bin-canvas")!;
const audio = new CatBinAudio();
const motion = new BinMotion();
const reducedMotion = matchMedia("(prefers-reduced-motion: reduce)");
let hover = false;
let focused = false;
let dragOver = false;
let dragged = false;
let busy = false;
let happyUntil = 0;
let previous = performance.now();
let binModel: THREE.Group | undefined;
let lidMixer: THREE.AnimationMixer | undefined;
let lidAction: THREE.AnimationAction | undefined;
let customModel = false;
let modelBaseScale = 1;
let modelLoadEpoch = 0;
const eyes: THREE.Object3D[] = [];
const happyEyes: THREE.Object3D[] = [];
const eyeMaterials = new Set<THREE.MeshStandardMaterial>();

const renderer = new THREE.WebGLRenderer({ canvas, alpha: true, antialias: true });
renderer.setPixelRatio(Math.min(Math.max(devicePixelRatio, 1) * 1.5, 3));
function resizeRenderer(): void {
  renderer.setSize(button.clientWidth || 76, button.clientHeight || 88, false);
}
resizeRenderer();
new ResizeObserver(resizeRenderer).observe(button);
renderer.setClearColor(0, 0);
renderer.outputColorSpace = THREE.SRGBColorSpace;
renderer.toneMapping = THREE.ACESFilmicToneMapping;
renderer.toneMappingExposure = .87;
const scene = new THREE.Scene();
scene.add(new THREE.HemisphereLight(0xffebce, 0x5e626e, 1.15));
const light = new THREE.DirectionalLight(0xffe2b5, 2.8);
light.position.set(-3, 5, 6);
scene.add(light);
const fill = new THREE.DirectionalLight(0xd9e6ff, .95);
fill.position.set(4, 2, -2);
scene.add(fill);
const pmrem = new THREE.PMREMGenerator(renderer);
const room = new RoomEnvironment();
const environment = pmrem.fromScene(room, .04);
scene.environment = environment.texture;
scene.environmentIntensity = .45;
room.dispose();
pmrem.dispose();
const camera = new THREE.OrthographicCamera(-1.123, 1.123, 1.30, -1.30, .01, 20);
camera.position.set(-2.3, 2.1, 5.8);
camera.lookAt(0, 1.04, 0);
camera.updateMatrixWorld();

button.addEventListener("pointerenter", () => { hover = true; });
button.addEventListener("pointerleave", () => { hover = false; });
button.addEventListener("focus", () => { focused = button.matches(":focus-visible"); });
button.addEventListener("blur", () => { focused = false; });
button.addEventListener("pointerdown", () => { dragged = false; focused = false; audio.unlock(); });
button.addEventListener("keydown", event => {
  if (event.key === "Enter" || event.key === " ") audio.unlock();
});
button.addEventListener("pointermove", event => {
  if ((event.buttons & 1) !== 0 && !dragged && !dragOver && !busy) {
    dragged = true;
    if (native) void invoke("start_bin_drag").catch(console.warn);
  }
});
button.addEventListener("dblclick", event => {
  event.preventDefault();
  if (native) void invoke("open_recycle_bin").catch(console.warn);
});
button.addEventListener("contextmenu", event => {
  event.preventDefault();
  if (native) void invoke("show_bin_menu").catch(console.warn);
});

if (native) {
  void listen<BinSettings>("bin://settings", ({ payload }) => {
    audio.enabled = payload.soundEnabled;
  });
  void invoke<BinSettings>("get_settings")
    .then(settings => { audio.enabled = settings.soundEnabled; })
    .catch(console.warn);
  void listen("bin://model-changed", () => { void loadModel(); });
  void listen("tauri://drag-enter", () => { dragOver = true; });
  void listen("tauri://drag-leave", () => { dragOver = false; });
  void listen("tauri://drag-drop", () => { dragOver = false; busy = true; });
  void listen<BinResult>("bin://result", ({ payload }) => {
    busy = false;
    dragOver = false;
    if (payload.ok) {
      happyUntil = performance.now() + 1300;
      audio.play("receive");
      button.title = "已移至 Windows 回收站 · 双击打开回收站 · 右键设置";
    } else if (payload.error) {
      button.title = `回收失败：${payload.error}`;
    } else if (payload.cancelled) {
      button.title = "已取消删除 · 拖入文件或文件夹 · 右键设置";
    }
  });
  window.setInterval(() => {
    void invoke<{ x: number; y: number }>("cursor_position_local")
      .then(point => { hover = point.x >= 0 && point.x < button.clientWidth && point.y >= 0 && point.y < button.clientHeight; })
      .catch(() => undefined);
  }, 80);
}

function disposeModel(root: THREE.Object3D): void {
  root.traverse(object => {
    if (!(object instanceof THREE.Mesh)) return;
    object.geometry.dispose();
    const materials = Array.isArray(object.material) ? object.material : [object.material];
    for (const material of materials) {
      for (const value of Object.values(material)) {
        if (value instanceof THREE.Texture && value !== environment.texture) value.dispose();
      }
      material.dispose();
    }
  });
}

function mountModel(gltf: GLTF, isCustom: boolean): void {
  const holder = new THREE.Group();
  holder.add(gltf.scene);
  let nextScale = 1;
  if (isCustom) {
    const box = new THREE.Box3().setFromObject(holder);
    const size = box.getSize(new THREE.Vector3());
    const longest = Math.max(size.x, size.y, size.z);
    if (box.isEmpty() || !Number.isFinite(longest) || longest <= 0) {
      disposeModel(holder);
      throw new Error("模型没有可显示的 3D 几何体");
    }
    nextScale = 1.7 / longest;
    holder.scale.setScalar(nextScale);
    holder.updateMatrixWorld(true);
    const center = new THREE.Box3().setFromObject(holder).getCenter(new THREE.Vector3());
    holder.position.set(-center.x, 1.04 - center.y, -center.z);
  }
  if (binModel) {
    scene.remove(binModel);
    lidMixer?.stopAllAction();
    disposeModel(binModel);
  }
  binModel = holder;
  customModel = isCustom;
  modelBaseScale = nextScale;
  eyes.length = 0;
  happyEyes.length = 0;
  eyeMaterials.clear();
  scene.add(holder);
  if (!isCustom) {
    gltf.scene.traverse(object => {
      if (/^Eye_[LR]$/.test(object.name)) eyes.push(object);
      if (/^HappyEye_[LR]$/.test(object.name)) { happyEyes.push(object); object.visible = false; }
      if (object instanceof THREE.Mesh) {
        const materials = Array.isArray(object.material) ? object.material : [object.material];
        for (const material of materials) {
          if (material instanceof THREE.MeshStandardMaterial && material.name === "EyeWarm_Emissive") {
            eyeMaterials.add(material);
          }
        }
      }
    });
  }
  lidMixer = new THREE.AnimationMixer(gltf.scene);
  const clip = isCustom ? gltf.animations[0] : gltf.animations.find(animation => animation.name === "BinOpen");
  lidAction = undefined;
  if (clip) {
    lidAction = lidMixer.clipAction(clip);
    lidAction.setLoop(THREE.LoopOnce, 1);
    lidAction.clampWhenFinished = true;
    lidAction.play();
    lidAction.paused = true;
  }
  button.classList.add("model-ready");
}

async function loadModel(): Promise<void> {
  const epoch = ++modelLoadEpoch;
  let isCustom = false;
  try {
    const settings = native ? await invoke<BinSettings>("get_settings") : undefined;
    isCustom = Boolean(settings?.modelName);
    const loader = new GLTFLoader();
    const gltf = isCustom
      ? await loader.parseAsync(Uint8Array.from(atob(await invoke<string>("read_active_model")), char => char.charCodeAt(0)).buffer, "")
      : await loader.loadAsync("/game/props/bin-cat.glb");
    if (epoch !== modelLoadEpoch) { disposeModel(gltf.scene); return; }
    mountModel(gltf, isCustom);
    button.title = isCustom
      ? `${settings?.modelName} · 拖入文件回收 · 右键更换模型`
      : "猫咪回收站 · 拖入文件回收 · 右键设置";
  } catch (error) {
    if (epoch !== modelLoadEpoch) return;
    console.error(error);
    button.title = `模型加载失败：${String(error)}`;
    if (isCustom && native) {
      void invoke("model_load_failed", { reason: String(error) }).catch(console.error);
    }
  }
}
void loadModel();

function frame(now: number): void {
  const dt = Math.min((now - previous) / 1000, .05);
  previous = now;
  const active = hover || focused || dragOver || busy || now < happyUntil;
  const sound = motion.step(dt, active, reducedMotion.matches);
  const openness = motion.openness;
  if (sound && binModel) audio.play(sound);
  if (customModel && binModel) binModel.scale.setScalar(modelBaseScale * (1 + openness * .045));
  if (lidAction && lidMixer) {
    lidAction.time = motion.pose * lidAction.getClip().duration;
    lidMixer.update(0);
  }
  button.dataset.openness = openness.toFixed(3);
  button.dataset.expression = now < happyUntil ? "happy" : openness > .1 ? "awake" : "idle";
  const happy = now < happyUntil;
  const blink = !reducedMotion.matches && !happy && now % 6100 > 5910;
  for (const eye of eyes) { eye.visible = !happy; eye.scale.y = blink ? .14 : 1; }
  for (const eye of happyEyes) eye.visible = happy;
  for (const material of eyeMaterials) {
    material.color.set(happy || openness > .1 ? 0xffcd85 : 0x71695f);
    material.emissive.set(0xffb84e);
    material.emissiveIntensity = happy ? 2.3 : openness * 2;
  }
  button.setAttribute("aria-label", openness > .5 ? "猫耳回收站，盖子已打开" : "猫耳回收站，悬浮打开");
  renderer.render(scene, camera);
  requestAnimationFrame(frame);
}
requestAnimationFrame(frame);
window.addEventListener("pagehide", () => { audio.dispose(); environment.dispose(); }, { once: true });
