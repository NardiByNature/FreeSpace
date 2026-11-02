import { F, type Core, type Sim } from "../wasm.js";
import { describe, parseEvents, type EventRow } from "./chronicle.js";

function fit(canvas: HTMLCanvasElement): CanvasRenderingContext2D | null {
  const dpr = window.devicePixelRatio || 1;
  const w = canvas.clientWidth;
  const h = canvas.clientHeight;
  if (w === 0 || h === 0) return null;
  if (canvas.width !== Math.floor(w * dpr) || canvas.height !== Math.floor(h * dpr)) {
    canvas.width = Math.floor(w * dpr);
    canvas.height = Math.floor(h * dpr);
  }
  const ctx = canvas.getContext("2d");
  if (!ctx) return null;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, w, h);
  return ctx;
}

/** Three small time-series panels fed straight from the history ring buffer. */
export class Charts {
  private lastKey = "";

  constructor(
    private readonly pop: HTMLCanvasElement,
    private readonly hue: HTMLCanvasElement,
    private readonly approach: HTMLCanvasElement,
  ) {}

  draw(core: Core, sim: Sim, force = false): void {
    const len = sim.history_len();
    const key = `${len}:${sim.history_head()}:${this.pop.clientWidth}:${sim.event_total()}`;
    if (!force && key === this.lastKey) return;
    this.lastKey = key;
    const hist = core.history(sim);
    const stride = core.layout.sampleStride;
    const rows: number[] = [];
    for (let i = 0; i < len; i++) rows.push(core.historyRowStart(sim, i));
    const events = parseEvents(sim.events(64));
    this.drawPop(hist, rows, stride, events);
    this.drawHue(hist, rows, core.layout.nFields, core.layout.hueBins);
    this.drawApproach(hist, rows);
  }

  private drawPop(h: Float32Array, rows: number[], _stride: number, events: EventRow[]): void {
    const ctx = fit(this.pop);
    if (!ctx) return;
    const w = this.pop.clientWidth;
    const hh = this.pop.clientHeight;
    const n = rows.length;
    if (n < 2) return;
    let max = 1;
    for (const r of rows) max = Math.max(max, h[r + F.pop], h[r + F.plants] / 4);
    const top = 18;
    const y = (v: number) => hh - 4 - (v / max) * (hh - top - 4);
    const x = (i: number) => (i / (n - 1)) * w;
    // Plants (scaled down) as a faint area behind.
    ctx.beginPath();
    ctx.moveTo(0, hh);
    for (let i = 0; i < n; i++) ctx.lineTo(x(i), y(h[rows[i] + F.plants] / 4));
    ctx.lineTo(w, hh);
    ctx.closePath();
    ctx.fillStyle = "rgba(96,196,112,0.10)";
    ctx.fill();
    // Stacked diet classes.
    const layers: [number, string][] = [
      [F.herb, "#60c470"],
      [F.omni, "#e3c25a"],
      [F.carn, "#e26b5a"],
    ];
    const base = new Float32Array(n);
    for (const [field, color] of layers) {
      ctx.beginPath();
      for (let i = 0; i < n; i++) ctx.lineTo(x(i), y(base[i] + h[rows[i] + field]));
      for (let i = n - 1; i >= 0; i--) ctx.lineTo(x(i), y(base[i]));
      ctx.closePath();
      ctx.fillStyle = color;
      ctx.globalAlpha = 0.75;
      ctx.fill();
      ctx.globalAlpha = 1;
      for (let i = 0; i < n; i++) base[i] += h[rows[i] + field];
    }
    // Chronicle marks: a hairline and a glyph where each event happened.
    const t0 = h[rows[0] + F.tick];
    const t1 = h[rows[n - 1] + F.tick];
    if (t1 > t0) {
      ctx.font = "10px ui-sans-serif, system-ui";
      ctx.textAlign = "center";
      let lastX = -20;
      let lane = 0;
      for (const e of events) {
        if (e.tick < t0) continue;
        const ex = ((e.tick - t0) / (t1 - t0)) * w;
        const d = describe(e);
        lane = ex - lastX < 10 ? (lane + 1) % 3 : 0;
        lastX = ex;
        ctx.strokeStyle = "rgba(214,222,232,0.22)";
        ctx.beginPath();
        ctx.moveTo(ex, top + 10 + lane * 11);
        ctx.lineTo(ex, hh);
        ctx.stroke();
        ctx.fillStyle = d.hue !== undefined ? `hsl(${d.hue * 360} 72% 62%)` : "#d6dee8";
        ctx.fillText(d.glyph, ex, top + 6 + lane * 11);
      }
    }
    ctx.fillStyle = "#7f8ea0";
    ctx.font = "10px ui-sans-serif, system-ui";
    ctx.textAlign = "right";
    ctx.fillText(`${Math.round(max)}`, w - 4, top - 2);
  }

  private drawHue(h: Float32Array, rows: number[], nFields: number, bins: number): void {
    const ctx = fit(this.hue);
    if (!ctx) return;
    const w = this.hue.clientWidth;
    const hh = this.hue.clientHeight;
    const n = rows.length;
    if (n < 1) return;
    const top = 18;
    const colW = Math.max(1, w / n);
    for (let i = 0; i < n; i++) {
      const r = rows[i] + nFields;
      let total = 0;
      for (let b = 0; b < bins; b++) total += h[r + b];
      if (total === 0) continue;
      let yPos = top;
      const x = (i / n) * w;
      for (let b = 0; b < bins; b++) {
        const c = h[r + b];
        if (c === 0) continue;
        const segH = (c / total) * (hh - top);
        ctx.fillStyle = `hsl(${(b / bins) * 360} 70% 58%)`;
        ctx.fillRect(x, yPos, colW + 0.5, segH + 0.5);
        yPos += segH;
      }
    }
  }

  private drawApproach(h: Float32Array, rows: number[]): void {
    const ctx = fit(this.approach);
    if (!ctx) return;
    const w = this.approach.clientWidth;
    const hh = this.approach.clientHeight;
    const n = rows.length;
    if (n < 2) return;
    const top = 18;
    // Light smoothing: the raw per-sample score is noisy.
    const vals = new Float32Array(n);
    for (let i = 0; i < n; i++) {
      let s = 0;
      let k = 0;
      for (let j = Math.max(0, i - 4); j <= Math.min(n - 1, i + 4); j++) {
        s += h[rows[j] + F.approach];
        k++;
      }
      vals[i] = s / k;
    }
    let max = 0.05;
    for (const v of vals) max = Math.max(max, v);
    const y = (v: number) => hh - 4 - (v / max) * (hh - top - 4);
    ctx.strokeStyle = "#1f2a36";
    ctx.beginPath();
    ctx.moveTo(0, y(0));
    ctx.lineTo(w, y(0));
    ctx.stroke();
    ctx.beginPath();
    for (let i = 0; i < n; i++) ctx.lineTo((i / (n - 1)) * w, y(vals[i]));
    ctx.strokeStyle = "#5ad1b3";
    ctx.lineWidth = 1.5;
    ctx.stroke();
    ctx.fillStyle = "#7f8ea0";
    ctx.font = "10px ui-sans-serif, system-ui";
    ctx.textAlign = "right";
    ctx.fillText(vals[n - 1].toFixed(2), w - 4, top - 2);
  }
}
