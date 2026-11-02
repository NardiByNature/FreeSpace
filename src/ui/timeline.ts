import type { Sim } from "../wasm.js";

export interface Checkpoint {
  tick: number;
  bytes: Uint8Array;
}

/** Ticks between checkpoints. */
export const CHECKPOINT_INTERVAL = 2000;
/** Most checkpoints kept; beyond this the older half is thinned. */
export const CHECKPOINT_CAP = 32;

/**
 * Keeps periodic snapshots so the dish can be rewound. Because snapshots
 * restore bit-for-bit, rewinding and leaving the world alone replays the
 * same future; any intervention makes a new branch.
 */
export class Timeline {
  points: Checkpoint[] = [];
  private lastBoundary = -1;

  reset(sim?: Sim): void {
    this.points = [];
    this.lastBoundary = -1;
    if (sim) this.capture(sim);
  }

  /** Call after stepping; captures once per interval boundary crossed. */
  maybeCapture(sim: Sim): void {
    const boundary = Math.floor(sim.tick() / CHECKPOINT_INTERVAL);
    if (boundary > this.lastBoundary) this.capture(sim);
  }

  capture(sim: Sim): void {
    const tick = sim.tick();
    this.lastBoundary = Math.floor(tick / CHECKPOINT_INTERVAL);
    // A new capture replaces any stale future from before a rewind.
    while (this.points.length > 0 && this.points[this.points.length - 1].tick >= tick) this.points.pop();
    this.points.push({ tick, bytes: sim.snapshot() });
    if (this.points.length > CHECKPOINT_CAP) this.thin();
  }

  /** Drop every other checkpoint from the older half so spacing doubles back there. */
  private thin(): void {
    const half = Math.floor(this.points.length / 2);
    const kept: Checkpoint[] = [];
    for (let i = 0; i < this.points.length; i++) {
      if (i < half && i % 2 === 1) continue;
      kept.push(this.points[i]);
    }
    this.points = kept;
  }

  /** The checkpoint to rewind to; discards everything after it. */
  take(index: number): Checkpoint | null {
    const cp = this.points[index];
    if (!cp) return null;
    this.points = this.points.slice(0, index + 1);
    this.lastBoundary = Math.floor(cp.tick / CHECKPOINT_INTERVAL);
    return cp;
  }

  /** Index of the newest checkpoint strictly before `tick`, or -1. */
  previousIndex(tick: number): number {
    for (let i = this.points.length - 1; i >= 0; i--) if (this.points[i].tick < tick) return i;
    return -1;
  }

  bytesHeld(): number {
    return this.points.reduce((s, p) => s + p.bytes.byteLength, 0);
  }
}

/** Draws the strip and maps clicks to checkpoints. */
export class TimelineView {
  private hover = -1;
  onRewind: ((index: number) => void) | null = null;

  constructor(private readonly canvas: HTMLCanvasElement, private readonly timeline: Timeline) {
    canvas.addEventListener("mousemove", (e) => {
      this.hover = this.hit(e);
      canvas.style.cursor = this.hover >= 0 ? "pointer" : "default";
      canvas.title = this.hover >= 0 ? `Rewind to tick ${this.timeline.points[this.hover].tick.toLocaleString()}` : "";
    });
    canvas.addEventListener("mouseleave", () => {
      this.hover = -1;
    });
    canvas.addEventListener("click", (e) => {
      const i = this.hit(e);
      if (i >= 0) this.onRewind?.(i);
    });
  }

  private hit(e: MouseEvent): number {
    const rect = this.canvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const w = rect.width;
    let best = -1;
    let bestD = 9;
    this.timeline.points.forEach((p, i) => {
      const px = this.xFor(p.tick, w);
      const d = Math.abs(px - x);
      if (d < bestD) {
        bestD = d;
        best = i;
      }
    });
    return best;
  }

  private currentTick = 1;

  private xFor(tick: number, w: number): number {
    const span = Math.max(this.currentTick, CHECKPOINT_INTERVAL);
    return 8 + (tick / span) * (w - 16);
  }

  draw(sim: Sim): void {
    const dpr = window.devicePixelRatio || 1;
    const w = this.canvas.clientWidth;
    const h = this.canvas.clientHeight;
    if (w === 0 || h === 0) return;
    if (this.canvas.width !== Math.floor(w * dpr) || this.canvas.height !== Math.floor(h * dpr)) {
      this.canvas.width = Math.floor(w * dpr);
      this.canvas.height = Math.floor(h * dpr);
    }
    const ctx = this.canvas.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);
    this.currentTick = sim.tick();
    const mid = h / 2;
    ctx.strokeStyle = "#1f2a36";
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.moveTo(8, mid);
    ctx.lineTo(w - 8, mid);
    ctx.stroke();
    this.timeline.points.forEach((p, i) => {
      const x = this.xFor(p.tick, w);
      const hot = i === this.hover;
      ctx.fillStyle = hot ? "#ffffff" : "#5ad1b3";
      ctx.beginPath();
      ctx.arc(x, mid, hot ? 5 : 3.5, 0, Math.PI * 2);
      ctx.fill();
    });
    // Live head.
    const hx = this.xFor(this.currentTick, w);
    ctx.fillStyle = "#d6dee8";
    ctx.beginPath();
    ctx.moveTo(hx, mid - 6);
    ctx.lineTo(hx + 5, mid);
    ctx.lineTo(hx, mid + 6);
    ctx.lineTo(hx - 5, mid);
    ctx.closePath();
    ctx.fill();
    ctx.fillStyle = "#7f8ea0";
    ctx.font = "10px ui-sans-serif, system-ui";
    ctx.textAlign = "left";
    ctx.textBaseline = "middle";
    ctx.fillText("t 0", 8, mid - 9);
    ctx.textAlign = "right";
    const label = this.timeline.points.length > 0 ? `${this.timeline.points.length} checkpoints · click one to rewind · [ ] to step` : "checkpoints appear every 2,000 ticks";
    ctx.fillText(label, w - 8, mid - 9);
  }
}
