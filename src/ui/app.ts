import { Camera } from "../render/camera.js";
import { Renderer } from "../render/renderer.js";
import { Core, Sim } from "../wasm.js";
import { Charts } from "./charts.js";
import { Inspector } from "./inspector.js";

const FRAME_BUDGET_MS = 13;
/** Simulation ticks per second at 1x, independent of display refresh rate. */
const BASE_TPS = 60;
const MAX_TICKS_PER_FRAME = 64;

function $<T extends HTMLElement>(id: string): T {
  const e = document.getElementById(id);
  if (!e) throw new Error(`missing #${id}`);
  return e as T;
}

/** Wires the simulation, renderer, and controls together and runs the frame loop. */
export class App {
  private sim: Sim;
  private cam: Camera;
  private readonly renderer: Renderer;
  private readonly inspector: Inspector;
  private readonly charts: Charts;
  private playing = true;
  /** Ticks per frame; 0 means as many as fit the budget. */
  private speed = 1;
  private selectedSlot = -1;
  private selectedId = 0;
  private follow = false;
  private frame = 0;
  private lastFrameTime = performance.now();
  private tickDebt = 0;
  private tpsWindow: { t: number; tick: number }[] = [];

  private readonly canvas = $<HTMLCanvasElement>("world");
  private readonly els = {
    play: $<HTMLButtonElement>("play"),
    tick: $("tick"),
    pop: $("pop"),
    plants: $("plants"),
    gen: $("gen"),
    tps: $("tps"),
    seed: $<HTMLInputElement>("seed"),
    file: $<HTMLInputElement>("file"),
  };

  constructor(private readonly core: Core, seed: number) {
    this.sim = new Sim(seed);
    this.cam = new Camera(this.sim.width(), this.sim.height());
    this.renderer = new Renderer(this.canvas);
    this.inspector = new Inspector($("side"));
    this.inspector.onFollowChange = (on) => {
      this.follow = on;
    };
    this.charts = new Charts($<HTMLCanvasElement>("popchart"), $<HTMLCanvasElement>("huechart"), $<HTMLCanvasElement>("approachchart"));
    this.els.seed.value = String(seed);
    this.bindControls();
    this.bindCanvas();
    this.layout();
    this.cam.fit();
    window.addEventListener("resize", () => this.layout());
    // Handy from the devtools console.
    (window as unknown as { tidepool: App }).tidepool = this;
    requestAnimationFrame(() => this.loop());
  }

  /** The live simulation handle (replaced on new world / load). */
  get world(): Sim {
    return this.sim;
  }

  get camera(): Camera {
    return this.cam;
  }

  private layout(): void {
    const rect = this.canvas.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    this.renderer.resize(rect.width, rect.height, dpr);
    this.cam.resize(rect.width, rect.height);
    this.charts.draw(this.core, this.sim, true);
  }

  private newWorld(seed: number): void {
    this.sim.free();
    this.sim = new Sim(seed);
    this.cam = new Camera(this.sim.width(), this.sim.height());
    this.cam.resize(this.canvas.clientWidth, this.canvas.clientHeight);
    this.cam.fit();
    this.selectedSlot = -1;
    this.inspector.reset();
    this.els.seed.value = String(seed);
    this.tpsWindow = [];
    history.replaceState(null, "", `#seed=${seed}`);
  }

  private bindControls(): void {
    this.els.play.addEventListener("click", () => this.togglePlay());
    for (const b of document.querySelectorAll<HTMLButtonElement>("#speed button")) {
      b.addEventListener("click", () => {
        this.speed = Number(b.dataset.s);
        document.querySelectorAll("#speed button").forEach((x) => x.classList.toggle("on", x === b));
      });
    }
    $("new").addEventListener("click", () => {
      const raw = this.els.seed.value.trim();
      const seed = raw === "" ? (Math.random() * 0xffffffff) >>> 0 : (Number(raw) >>> 0) || 0;
      this.newWorld(seed);
    });
    $("life").addEventListener("click", () => this.sim.spawn_founders(50));
    $("save").addEventListener("click", () => this.save());
    $("load").addEventListener("click", () => this.els.file.click());
    this.els.file.addEventListener("change", () => {
      const f = this.els.file.files?.[0];
      if (f) void this.load(f);
      this.els.file.value = "";
    });
    window.addEventListener("keydown", (e) => {
      if (e.target instanceof HTMLInputElement) return;
      if (e.code === "Space") {
        e.preventDefault();
        this.togglePlay();
      } else if (e.key === "Escape") {
        this.selectedSlot = -1;
        this.inspector.reset();
      } else if (e.key === "f") {
        this.cam.fit();
      }
    });
  }

  private togglePlay(): void {
    this.playing = !this.playing;
    this.els.play.textContent = this.playing ? "Pause" : "Play";
    this.tpsWindow = [];
  }

  private bindCanvas(): void {
    let dragging = false;
    let moved = 0;
    let lastX = 0;
    let lastY = 0;
    this.canvas.addEventListener("pointerdown", (e) => {
      dragging = true;
      moved = 0;
      lastX = e.clientX;
      lastY = e.clientY;
      this.canvas.setPointerCapture(e.pointerId);
    });
    this.canvas.addEventListener("pointermove", (e) => {
      if (!dragging) return;
      const dx = e.clientX - lastX;
      const dy = e.clientY - lastY;
      moved += Math.abs(dx) + Math.abs(dy);
      lastX = e.clientX;
      lastY = e.clientY;
      if (moved > 3) {
        this.cam.pan(dx, dy);
        this.follow = false;
        if (this.inspector.followBox) this.inspector.followBox.checked = false;
      }
    });
    this.canvas.addEventListener("pointerup", (e) => {
      dragging = false;
      if (moved <= 3) this.pick(e);
    });
    this.canvas.addEventListener("wheel", (e) => {
      e.preventDefault();
      const rect = this.canvas.getBoundingClientRect();
      const factor = Math.exp(-e.deltaY * 0.001);
      this.cam.zoomAt(e.clientX - rect.left, e.clientY - rect.top, factor);
    }, { passive: false });
  }

  private pick(e: PointerEvent): void {
    const rect = this.canvas.getBoundingClientRect();
    const wx = this.cam.worldX(e.clientX - rect.left);
    const wy = this.cam.worldY(e.clientY - rect.top);
    const r = 12 / this.cam.zoom + 10;
    const slot = this.sim.pick(wx, wy, r);
    if (slot >= 0) {
      this.selectedSlot = slot;
      this.selectedId = this.sim.creature_id(slot);
      this.inspector.reset();
    } else {
      this.selectedSlot = -1;
      this.inspector.reset();
    }
  }

  private save(): void {
    const bytes = this.sim.snapshot();
    const blob = new Blob([new Uint8Array(bytes).buffer as ArrayBuffer], { type: "application/octet-stream" });
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = `tidepool-${this.sim.seed()}-t${this.sim.tick()}.tdpl`;
    a.click();
    setTimeout(() => URL.revokeObjectURL(a.href), 1000);
  }

  private async load(file: File): Promise<void> {
    const buf = new Uint8Array(await file.arrayBuffer());
    let next: Sim;
    try {
      next = Sim.restore(buf);
    } catch (err) {
      alert(`Could not load snapshot: ${err instanceof Error ? err.message : String(err)}`);
      return;
    }
    this.sim.free();
    this.sim = next;
    this.cam = new Camera(this.sim.width(), this.sim.height());
    this.cam.resize(this.canvas.clientWidth, this.canvas.clientHeight);
    this.cam.fit();
    this.selectedSlot = -1;
    this.inspector.reset();
    this.els.seed.value = String(this.sim.seed());
    this.tpsWindow = [];
  }

  private stepFrame(now: number): void {
    const dt = Math.min(0.25, (now - this.lastFrameTime) / 1000);
    this.lastFrameTime = now;
    if (!this.playing) {
      this.tickDebt = 0;
      return;
    }
    if (this.speed > 0) {
      // Real-time pacing: owe BASE_TPS * speed ticks per second, whatever the refresh rate.
      this.tickDebt += dt * BASE_TPS * this.speed;
      const n = Math.min(MAX_TICKS_PER_FRAME, Math.floor(this.tickDebt));
      if (n > 0) {
        this.sim.step(n);
        this.tickDebt -= n;
      }
      return;
    }
    const start = performance.now();
    let chunk = 4;
    while (performance.now() - start < FRAME_BUDGET_MS) {
      this.sim.step(chunk);
      chunk = Math.min(MAX_TICKS_PER_FRAME, chunk * 2);
    }
  }

  private loop(): void {
    this.frame++;
    this.stepFrame(performance.now());
    this.sim.sync();

    // Selection bookkeeping: slots are recycled, so confirm the id still matches.
    if (this.selectedSlot >= 0 && (!this.sim.creature_alive(this.selectedSlot) || this.sim.creature_id(this.selectedSlot) !== this.selectedId)) {
      this.selectedSlot = -1;
      this.inspector.reset();
    }
    let selX = 0;
    let selY = 0;
    let selSense = 0;
    if (this.selectedSlot >= 0) {
      const d = this.sim.inspect(this.selectedSlot);
      const L = this.core.layout;
      selSense = d[L.iTraits + 2];
      // Position comes from the packed view; find our row by slot.
      const v = this.core.creatures(this.sim);
      const n = this.sim.creatures_len();
      for (let i = 0; i < n; i++) {
        const o = i * L.creatureStride;
        if (v[o + 7] === this.selectedSlot) {
          selX = v[o];
          selY = v[o + 1];
          break;
        }
      }
      if (this.follow) this.cam.centerOn(selX, selY);
    }

    this.renderer.draw(this.core, this.sim, this.cam, this.selectedSlot);
    if (this.selectedSlot >= 0) {
      this.renderer.drawSenseRing(this.cam, selX, selY, selSense);
      this.inspector.showCreature(this.core, this.sim, this.selectedSlot, this.follow);
    } else if (this.frame % 6 === 0) {
      this.inspector.showWorld(this.core, this.sim);
    }
    if (this.frame % 8 === 0) {
      this.charts.draw(this.core, this.sim);
      this.updateReadouts();
    }
    requestAnimationFrame(() => this.loop());
  }

  private updateReadouts(): void {
    const now = performance.now();
    const tick = this.sim.tick();
    this.tpsWindow.push({ t: now, tick });
    while (this.tpsWindow.length > 2 && now - this.tpsWindow[0].t > 1500) this.tpsWindow.shift();
    const first = this.tpsWindow[0];
    const tps = this.tpsWindow.length > 1 ? ((tick - first.tick) / (now - first.t)) * 1000 : 0;
    this.els.tick.textContent = tick.toLocaleString();
    this.els.pop.textContent = this.sim.population().toLocaleString();
    this.els.plants.textContent = this.sim.plant_count().toLocaleString();
    this.els.gen.textContent = String(this.sim.max_generation());
    this.els.tps.textContent = this.playing ? String(Math.round(tps)) : "–";
  }
}
