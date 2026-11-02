import { Camera } from "../render/camera.js";
import { Renderer } from "../render/renderer.js";
import { Core, Sim } from "../wasm.js";
import { Charts } from "./charts.js";
import { Inspector } from "./inspector.js";
import { Timeline, TimelineView } from "./timeline.js";
import { toast } from "./toast.js";

const FRAME_BUDGET_MS = 13;
/** Longest fast-forward a moment link may ask for. */
const MAX_FAST_FORWARD = 2_000_000;
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
  readonly timeline = new Timeline();
  private readonly timelineView: TimelineView;
  private playing = true;
  /** Ticks per frame; 0 means as many as fit the budget. */
  private speed = 1;
  private selectedSlot = -1;
  private selectedId = 0;
  private follow = false;
  /** Recent positions of the selected creature, for the trail. */
  private trail: number[] = [];
  /** When > 0, run at full speed until this tick (moment links). */
  private fastTo = 0;
  private speedBeforeFast = 1;
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
    season: $("season"),
    seasonWord: $("seasonword"),
    seed: $<HTMLInputElement>("seed"),
    sexual: $<HTMLInputElement>("sexual"),
    file: $<HTMLInputElement>("file"),
  };

  constructor(private readonly core: Core, seed: number, sexual: boolean, targetTick = 0) {
    this.sim = new Sim(seed, sexual);
    this.cam = new Camera(this.sim.width(), this.sim.height());
    this.renderer = new Renderer(this.canvas);
    this.inspector = new Inspector($("side"));
    this.inspector.onFollowChange = (on) => {
      this.follow = on;
    };
    this.inspector.onSpawnCode = (code, name) => this.spawnCode(code, name);
    this.inspector.onSelectSlot = (kind) => {
      const slot = this.sim.champion(kind);
      if (slot >= 0) this.select(slot);
    };
    window.addEventListener("hashchange", () => {
      const m = /seed=(\d+)/.exec(location.hash);
      if (!m) return;
      const seed = Number(m[1]) >>> 0;
      const sexual = /(^|[#&])sexual(&|$)/.test(location.hash);
      const tm = /(^|[#&])t=(\d+)/.exec(location.hash);
      const t = tm ? Number(tm[2]) : 0;
      if (seed !== this.sim.seed() || sexual !== this.sim.sexual() || (t > 0 && t < this.sim.tick())) {
        this.els.sexual.checked = sexual;
        this.newWorld(seed);
      }
      if (t > 0) this.fastForwardTo(t);
    });
    $("link").addEventListener("click", () => {
      const url = this.momentLink();
      navigator.clipboard
        .writeText(url)
        .then(() => toast(`Link copied. It replays seed ${this.sim.seed()} to t ${this.sim.tick().toLocaleString()}.`, 4000))
        .catch(() => window.prompt("Copy this link:", url));
    });
    this.charts = new Charts($<HTMLCanvasElement>("popchart"), $<HTMLCanvasElement>("huechart"), $<HTMLCanvasElement>("approachchart"));
    this.timelineView = new TimelineView($<HTMLCanvasElement>("timeline"), this.timeline);
    this.timelineView.onRewind = (i) => this.rewind(i);
    this.timeline.reset(this.sim);
    this.els.seed.value = String(seed);
    this.els.sexual.checked = sexual;
    this.bindControls();
    this.bindCanvas();
    this.layout();
    this.cam.fit();
    new ResizeObserver(() => this.layout()).observe(this.canvas);
    window.addEventListener("resize", () => this.layout());
    // Handy from the devtools console.
    (window as unknown as { tidepool: App }).tidepool = this;
    if (targetTick > 0) this.fastForwardTo(targetTick);
    requestAnimationFrame(() => this.loop());
  }

  /** Run flat out until `tick`, then drop back to the previous speed. */
  fastForwardTo(tick: number): void {
    if (tick > MAX_FAST_FORWARD) {
      toast(`That link asks for t ${tick.toLocaleString()}; the most a link will replay is ${MAX_FAST_FORWARD.toLocaleString()}.`, 8000);
      tick = MAX_FAST_FORWARD;
    }
    if (tick <= this.sim.tick()) return;
    if (this.fastTo === 0) this.speedBeforeFast = this.speed || 1;
    this.fastTo = tick;
    this.playing = true;
    this.els.play.textContent = "Pause";
    document.querySelectorAll<HTMLButtonElement>("#speed button").forEach((b) => (b.disabled = true));
  }

  private endFastForward(): void {
    this.fastTo = 0;
    this.speed = this.speedBeforeFast;
    document.querySelectorAll<HTMLButtonElement>("#speed button").forEach((b) => {
      b.disabled = false;
      b.classList.toggle("on", Number(b.dataset.s) === this.speed);
    });
    toast(`Arrived at t ${this.sim.tick().toLocaleString()}.`, 3000);
  }

  private momentLink(): string {
    const rule = this.sim.sexual() ? "&sexual" : "";
    return `${location.origin}${location.pathname}#seed=${this.sim.seed()}${rule}&t=${this.sim.tick()}`;
  }

  /** The live simulation handle (replaced on new world / load). */
  get world(): Sim {
    return this.sim;
  }

  get camera(): Camera {
    return this.cam;
  }

  private layout(): void {
    const w = this.canvas.clientWidth;
    const h = this.canvas.clientHeight;
    if (w === 0 || h === 0) return;
    const dpr = window.devicePixelRatio || 1;
    this.renderer.resize(w, h, dpr);
    this.cam.resize(w, h);
    this.charts.draw(this.core, this.sim, true);
  }

  /** Swap in a new simulation handle; `keepView` preserves camera and timeline (rewind). */
  private replaceSim(next: Sim, keepView: boolean): void {
    const old = this.sim;
    this.sim = next;
    old.free();
    if (!keepView) {
      this.cam = new Camera(this.sim.width(), this.sim.height());
      this.cam.resize(this.canvas.clientWidth, this.canvas.clientHeight);
      this.cam.fit();
      this.timeline.reset(this.sim);
    }
    this.selectedSlot = -1;
    this.inspector.reset();
    this.els.seed.value = String(this.sim.seed());
    this.els.sexual.checked = this.sim.sexual();
    this.tpsWindow = [];
    this.tickDebt = 0;
    history.replaceState(null, "", `#seed=${this.sim.seed()}${this.sim.sexual() ? "&sexual" : ""}`);
  }

  private newWorld(seed: number): void {
    this.replaceSim(new Sim(seed, this.els.sexual.checked), false);
  }

  /** Rewind to checkpoint `index`. The world then replays identically unless touched. */
  rewind(index: number): void {
    const cp = this.timeline.take(index);
    if (!cp) return;
    let next: Sim;
    try {
      next = Sim.restore(cp.bytes);
    } catch (err) {
      toast(`Could not rewind: ${err instanceof Error ? err.message : String(err)}`);
      return;
    }
    this.replaceSim(next, true);
    toast(`Rewound to t ${cp.tick.toLocaleString()}. Left alone, the future replays exactly; touch anything and it branches.`);
  }

  private spawnCode(code: string, name: string): void {
    const placed = this.sim.spawn_code(code, 20);
    if (placed === 0) {
      toast("That is not a genome code, or the dish is full.", 4000);
    } else {
      toast(`Placed ${placed} copies of ${name}.`, 3000);
    }
  }

  private bindControls(): void {
    this.els.play.addEventListener("click", () => this.togglePlay());
    for (const b of document.querySelectorAll<HTMLButtonElement>("#speed button")) {
      b.addEventListener("click", () => {
        if (this.fastTo > 0) return;
        this.speed = Number(b.dataset.s);
        document.querySelectorAll("#speed button").forEach((x) => x.classList.toggle("on", x === b));
      });
    }
    $("new").addEventListener("click", () => {
      const raw = this.els.seed.value.trim();
      const seed = raw === "" ? (Math.random() * 0xffffffff) >>> 0 : (Number(raw) >>> 0) || 0;
      this.newWorld(seed);
    });
    $("life").addEventListener("click", () => {
      this.sim.spawn_founders(50);
      toast("The tide brought 50 newcomers.", 2500);
    });
    $("paste").addEventListener("click", () => {
      const code = window.prompt("Paste a genome code:");
      if (code) this.spawnCode(code, "pasted genome");
    });
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
      } else if (e.key === "[") {
        const i = this.timeline.previousIndex(this.sim.tick());
        if (i >= 0) this.rewind(i);
      } else if (e.key === "]") {
        // After a rewind the future is discarded, so there is nothing to step to;
        // the key exists so the pair feels complete and for a future branch list.
        toast("Nothing ahead of now. Let it run.", 2500);
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
      this.select(slot);
    } else {
      this.selectedSlot = -1;
      this.inspector.reset();
    }
  }

  private select(slot: number): void {
    this.selectedSlot = slot;
    this.selectedId = this.sim.creature_id(slot);
    this.trail = [];
    this.inspector.reset();
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
    this.replaceSim(next, false);
  }

  private stepFrame(now: number): void {
    const dt = Math.min(0.25, (now - this.lastFrameTime) / 1000);
    this.lastFrameTime = now;
    if (!this.playing) {
      this.tickDebt = 0;
      return;
    }
    if (this.fastTo > 0) {
      const start = performance.now();
      let chunk = 8;
      while (performance.now() - start < FRAME_BUDGET_MS && this.sim.tick() < this.fastTo) {
        this.sim.step(Math.min(chunk, this.fastTo - this.sim.tick()));
        chunk = Math.min(MAX_TICKS_PER_FRAME, chunk * 2);
      }
      if (this.sim.tick() >= this.fastTo) this.endFastForward();
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
    const before = this.sim.tick();
    this.stepFrame(performance.now());
    if (this.sim.tick() !== before) this.timeline.maybeCapture(this.sim);
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
      const t = this.trail;
      if (t.length < 2 || t[t.length - 2] !== selX || t[t.length - 1] !== selY) {
        t.push(selX, selY);
        if (t.length > 480) t.splice(0, t.length - 480);
      }
    }

    this.renderer.draw(this.core, this.sim, this.cam, this.selectedSlot);
    if (this.selectedSlot >= 0) {
      this.renderer.drawTrail(this.cam, this.trail);
      this.renderer.drawSenseRing(this.cam, selX, selY, selSense);
      this.inspector.showCreature(this.core, this.sim, this.selectedSlot, this.follow);
    } else if (this.frame % 6 === 0) {
      this.inspector.showWorld(this.core, this.sim);
    }
    if (this.frame % 8 === 0) {
      this.charts.draw(this.core, this.sim);
      this.timelineView.draw(this.sim);
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
    const phase = this.sim.season_phase();
    const [glyph, word] = phase < 0.125 || phase >= 0.875 ? ["☀", "high summer"] : phase < 0.375 ? ["☁", "autumn"] : phase < 0.625 ? ["❄", "deep winter"] : ["☂", "spring"];
    const label = `${glyph} `;
    if (this.els.season.firstChild && this.els.season.firstChild.textContent !== label) this.els.season.firstChild.textContent = label;
    if (this.els.seasonWord.textContent !== word) this.els.seasonWord.textContent = word;
    if (this.fastTo > 0) {
      const pct = Math.min(100, Math.round((tick / this.fastTo) * 100));
      toast(`Fast-forwarding to t ${this.fastTo.toLocaleString()} … ${pct}%`, 60_000);
    }
  }
}
