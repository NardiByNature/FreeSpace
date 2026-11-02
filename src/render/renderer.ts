import type { Camera } from "./camera.js";
import { C, type Core, type Sim } from "../wasm.js";

const HUE_BUCKETS = 48;

/**
 * Canvas 2D renderer. Creatures are batched by hue bucket so there is one
 * fill per colour rather than one per creature, which keeps 10k bodies at
 * 60 fps. The nutrient field is painted to a tiny offscreen canvas and tiled.
 */
export class Renderer {
  private readonly ctx: CanvasRenderingContext2D;
  private field: HTMLCanvasElement;
  private fieldCtx: CanvasRenderingContext2D;
  private fieldImage: ImageData | null = null;
  private readonly hueFills: string[] = [];
  private readonly hueFillsDim: string[] = [];
  private readonly paths: Path2D[] = [];
  private readonly pathsDim: Path2D[] = [];
  private lastFieldTick = -1;
  dpr = 1;

  constructor(readonly canvas: HTMLCanvasElement) {
    const ctx = canvas.getContext("2d", { alpha: false });
    if (!ctx) throw new Error("no 2d context");
    this.ctx = ctx;
    this.field = document.createElement("canvas");
    const fctx = this.field.getContext("2d");
    if (!fctx) throw new Error("no 2d context");
    this.fieldCtx = fctx;
    for (let b = 0; b < HUE_BUCKETS; b++) {
      const h = Math.round((b / HUE_BUCKETS) * 360);
      this.hueFills.push(`hsl(${h} 75% 62%)`);
      this.hueFillsDim.push(`hsl(${h} 35% 30%)`);
      this.paths.push(new Path2D());
      this.pathsDim.push(new Path2D());
    }
  }

  /** Size the backing store to the canvas's CSS box; CSS owns the layout. */
  resize(w: number, h: number, dpr: number): void {
    this.dpr = dpr;
    const bw = Math.max(1, Math.floor(w * dpr));
    const bh = Math.max(1, Math.floor(h * dpr));
    if (this.canvas.width !== bw) this.canvas.width = bw;
    if (this.canvas.height !== bh) this.canvas.height = bh;
  }

  draw(core: Core, sim: Sim, cam: Camera, selectedSlot: number): void {
    const ctx = this.ctx;
    const dpr = this.dpr;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.fillStyle = "#0a1219";
    ctx.fillRect(0, 0, cam.viewW, cam.viewH);

    this.drawField(core, sim, cam);
    this.drawPlants(core, sim, cam);
    this.drawCreatures(core, sim, cam, selectedSlot);
  }

  private drawField(core: Core, sim: Sim, cam: Camera): void {
    const cols = sim.grid_cols();
    const rows = sim.grid_rows();
    if (this.field.width !== cols || this.field.height !== rows) {
      this.field.width = cols;
      this.field.height = rows;
      this.fieldImage = this.fieldCtx.createImageData(cols, rows);
      this.lastFieldTick = -1;
    }
    const tick = sim.tick();
    if (tick !== this.lastFieldTick && this.fieldImage) {
      this.lastFieldTick = tick;
      const level = core.nutrients(sim);
      const cap = sim.nutrient_capacity();
      const px = this.fieldImage.data;
      for (let i = 0; i < cols * rows; i++) {
        const t = Math.min(1, level[i] / cap);
        // Dark water to a faint mineral teal; stays quiet under the life on top.
        px[i * 4] = 10 + 16 * t;
        px[i * 4 + 1] = 18 + 62 * t;
        px[i * 4 + 2] = 25 + 54 * t;
        px[i * 4 + 3] = 255;
      }
      this.fieldCtx.putImageData(this.fieldImage, 0, 0);
    }
    const ctx = this.ctx;
    ctx.imageSmoothingEnabled = true;
    const wpx = cam.worldW * cam.zoom;
    const hpx = cam.worldH * cam.zoom;
    const ox = cam.screenX(0);
    const oy = cam.screenY(0);
    // The origin tile plus whichever neighbours are needed to cover the view.
    for (const kx of [-1, 0, 1]) {
      for (const ky of [-1, 0, 1]) {
        const x = ox + kx * wpx;
        const y = oy + ky * hpx;
        if (x > cam.viewW || y > cam.viewH || x + wpx < 0 || y + hpx < 0) continue;
        ctx.drawImage(this.field, x, y, wpx, hpx);
      }
    }
  }

  private drawPlants(core: Core, sim: Sim, cam: Camera): void {
    const ctx = this.ctx;
    const v = core.plants(sim);
    const stride = core.layout.plantStride;
    const n = sim.plants_len();
    const maxE = sim.plant_max_energy();
    const baseR = sim.plant_radius();
    const z = cam.zoom;
    const margin = baseR * z + 2;
    const path = new Path2D();
    const pathYoung = new Path2D();
    for (let i = 0; i < n; i++) {
      const o = i * stride;
      const sx = cam.screenX(v[o]);
      const sy = cam.screenY(v[o + 1]);
      if (sx < -margin || sy < -margin || sx > cam.viewW + margin || sy > cam.viewH + margin) continue;
      const e = v[o + 2] / maxE;
      const r = Math.max(0.6, baseR * Math.sqrt(e) * z);
      const p = e < 0.25 ? pathYoung : path;
      p.moveTo(sx + r, sy);
      p.arc(sx, sy, r, 0, Math.PI * 2);
    }
    ctx.fillStyle = "rgba(96, 196, 112, 0.92)";
    ctx.fill(path);
    ctx.fillStyle = "rgba(96, 196, 112, 0.45)";
    ctx.fill(pathYoung);
  }

  private drawCreatures(core: Core, sim: Sim, cam: Camera, selectedSlot: number): void {
    const ctx = this.ctx;
    const v = core.creatures(sim);
    const stride = core.layout.creatureStride;
    const n = sim.creatures_len();
    const z = cam.zoom;
    for (let b = 0; b < HUE_BUCKETS; b++) {
      this.paths[b] = new Path2D();
      this.pathsDim[b] = new Path2D();
    }
    const noses = new Path2D();
    let selX = -1;
    let selY = -1;
    let selR = 0;
    let selSense = 0;
    const margin = 12 * z + 2;
    for (let i = 0; i < n; i++) {
      const o = i * stride;
      const sx = cam.screenX(v[o + C.x]);
      const sy = cam.screenY(v[o + C.y]);
      if (sx < -margin || sy < -margin || sx > cam.viewW + margin || sy > cam.viewH + margin) continue;
      const r = Math.max(0.8, v[o + C.radius] * z);
      const hue = v[o + C.hue];
      const bucket = Math.min(HUE_BUCKETS - 1, Math.floor(hue * HUE_BUCKETS));
      const energy = v[o + C.energy];
      const p = energy < 0.2 ? this.pathsDim[bucket] : this.paths[bucket];
      p.moveTo(sx + r, sy);
      p.arc(sx, sy, r, 0, Math.PI * 2);
      if (z > 0.6) {
        // A nose along the heading; carnivores wear a longer one.
        const h = v[o + C.heading];
        const diet = v[o + C.diet];
        const len = r * (1.0 + 1.4 * diet);
        noses.moveTo(sx, sy);
        noses.lineTo(sx + Math.cos(h) * len, sy + Math.sin(h) * len);
      }
      if (v[o + C.slot] === selectedSlot) {
        selX = sx;
        selY = sy;
        selR = r;
        selSense = 0; // filled by caller via inspect; we draw the ring here
      }
    }
    for (let b = 0; b < HUE_BUCKETS; b++) {
      ctx.fillStyle = this.hueFillsDim[b];
      ctx.fill(this.pathsDim[b]);
      ctx.fillStyle = this.hueFills[b];
      ctx.fill(this.paths[b]);
    }
    if (z > 0.6) {
      ctx.strokeStyle = "rgba(255,255,255,0.55)";
      ctx.lineWidth = Math.max(1, 0.8 * z);
      ctx.stroke(noses);
    }
    if (selX >= 0) {
      ctx.strokeStyle = "rgba(255,255,255,0.9)";
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      ctx.arc(selX, selY, selR + 4, 0, Math.PI * 2);
      ctx.stroke();
      void selSense;
    }
  }

  /** Draw the selected creature's sense radius; called after `draw` with inspect data. */
  drawSenseRing(cam: Camera, x: number, y: number, sense: number): void {
    const ctx = this.ctx;
    ctx.strokeStyle = "rgba(255,255,255,0.25)";
    ctx.setLineDash([4, 4]);
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.arc(cam.screenX(x), cam.screenY(y), sense * cam.zoom, 0, Math.PI * 2);
    ctx.stroke();
    ctx.setLineDash([]);
  }
}
