import "./style.css";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import * as THREE from "three";
import { GLTFLoader } from "three/examples/jsm/loaders/GLTFLoader.js";
import { RoomEnvironment } from "three/examples/jsm/environments/RoomEnvironment.js";
import { CatBinAudio } from "./cat-bin-audio";
import { BinMotion } from "./bin-motion";

type BinSettings = { confirmDelete: boolean; soundEnabled: boolean; scalePercent: number };
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

async function boot(): Promise<void> {
  const gltf = await new GLTFLoader().loadAsync("/game/props/bin-cat.glb");
  binModel = gltf.scene;
  scene.add(gltf.scene);
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
  lidMixer = new THREE.AnimationMixer(gltf.scene);
  const clip = gltf.animations.find(animation => animation.name === "BinOpen");
  if (clip) {
    lidAction = lidMixer.clipAction(clip);
    lidAction.setLoop(THREE.LoopOnce, 1);
    lidAction.clampWhenFinished = true;
    lidAction.play();
    lidAction.paused = true;
  }
  button.classList.add("model-ready");
}
void boot().catch(error => {
  button.title = `垃圾桶模型加载失败：${String(error)}`;
  console.error(error);
});

function frame(now: number): void {
  const dt = Math.min((now - previous) / 1000, .05);
  previous = now;
  const active = hover || focused || dragOver || busy || now < happyUntil;
  const sound = motion.step(dt, active, reducedMotion.matches);
  const openness = motion.openness;
  if (sound && binModel) audio.play(sound);
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
