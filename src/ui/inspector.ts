import { F, I, type Core, type Sim } from "../wasm.js";

// Trait display ranges, mirroring the clamps in core/src/genome.rs.
const TRAIT_RANGES: [number, number][] = [
  [0.6, 2.5], // size
  [0.5, 3.0], // speed
  [16, 96], // sense
  [0, 1], // hue
  [0, 1], // diet
  [0.4, 0.95], // breed at
  [0.005, 0.2], // mutation
];

function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string, text?: string): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

/** Side panel: world summary when nothing is selected, one creature otherwise. */
export class Inspector {
  private mode: "world" | "creature" | null = null;
  private vals = new Map<string, HTMLElement>();
  private bars = new Map<string, HTMLElement>();
  private brain: HTMLCanvasElement | null = null;
  followBox: HTMLInputElement | null = null;
  onFollowChange: ((on: boolean) => void) | null = null;

  constructor(private readonly root: HTMLElement) {}

  showWorld(core: Core, sim: Sim): void {
    if (this.mode !== "world") this.buildWorld(core);
    const len = sim.history_len();
    if (len === 0) return;
    const h = core.history(sim);
    const r = core.historyRowStart(sim, len - 1);
    const set = (k: string, v: string) => {
      const e = this.vals.get(k);
      if (e && e.textContent !== v) e.textContent = v;
    };
    set("herb", `${h[r + F.herb]}`);
    set("omni", `${h[r + F.omni]}`);
    set("carn", `${h[r + F.carn]}`);
    set("births", `${h[r + F.births]} / ${h[r + F.deaths]}`);
    set("approach", h[r + F.approach].toFixed(3));
    set("size", h[r + F.meanSize].toFixed(2));
    set("speed", h[r + F.meanSpeed].toFixed(2));
    set("sense", h[r + F.meanSense].toFixed(1));
    set("diet", h[r + F.meanDiet].toFixed(2));
    const cap = sim.nutrient_capacity() * sim.grid_cols() * sim.grid_rows();
    set("soil", `${Math.round((h[r + F.nutrients] / cap) * 100)}%`);
  }

  private buildWorld(core: Core): void {
    this.mode = "world";
    this.root.replaceChildren();
    this.vals.clear();
    this.root.append(el("h2", undefined, "The dish"));
    this.root.append(
      el(
        "p",
        undefined,
        "Every dot is a creature steering with a small neural network it was born with. Green blobs are bushes that regrow from the soil. Nothing is scripted: bodies that find food breed, the rest starve, and the mix of brains and bodies drifts from there.",
      ),
    );
    this.root.append(el("p", undefined, "Click a creature to watch its brain. Drag to pan, scroll to zoom, space to pause."));
    this.root.append(el("h3", undefined, "Right now"));
    const kv = el("div", "kv");
    const add = (k: string, label: string) => {
      kv.append(el("span", undefined, label));
      const v = el("span", undefined, "–");
      this.vals.set(k, v);
      kv.append(v);
    };
    add("herb", "herbivores");
    add("omni", "omnivores");
    add("carn", "carnivores");
    add("births", "births / deaths");
    add("soil", "soil nutrients");
    add("approach", "food-seeking");
    this.root.append(kv);
    this.root.append(el("h3", undefined, "Average body"));
    const kv2 = el("div", "kv");
    const add2 = (k: string, label: string) => {
      kv2.append(el("span", undefined, label));
      const v = el("span", undefined, "–");
      this.vals.set(k, v);
      kv2.append(v);
    };
    add2("size", "size");
    add2("speed", "speed");
    add2("sense", "sense range");
    add2("diet", "diet (0 plant … 1 meat)");
    this.root.append(kv2);
    void core;
  }

  showCreature(core: Core, sim: Sim, slot: number, follow: boolean): void {
    if (this.mode !== "creature") this.buildCreature(core, follow);
    const d = sim.inspect(slot);
    const L = core.layout;
    const set = (k: string, v: string) => {
      const e = this.vals.get(k);
      if (e && e.textContent !== v) e.textContent = v;
    };
    const bar = (k: string, frac: number) => {
      const e = this.bars.get(k);
      if (e) e.style.width = `${Math.max(0, Math.min(1, frac)) * 100}%`;
    };
    set("id", `#${d[I.id]}`);
    set("gen", `${d[I.generation]}`);
    set("age", `${Math.round(d[I.age])} / ${Math.round(d[I.maxAge])}`);
    bar("age", d[I.age] / d[I.maxAge]);
    set("energy", `${d[I.energy].toFixed(1)} / ${d[I.maxEnergy].toFixed(0)}`);
    bar("energy", d[I.energy] / d[I.maxEnergy]);
    set("meals", `${d[I.meals]}`);
    const hue = d[L.iTraits + 3];
    const sw = this.vals.get("swatch");
    if (sw) sw.style.background = `hsl(${hue * 360} 75% 62%)`;
    for (let t = 0; t < L.nTraits; t++) {
      const v = d[L.iTraits + t];
      const [lo, hi] = TRAIT_RANGES[t] ?? [0, 1];
      set(`t${t}`, t === 2 ? v.toFixed(0) : v.toFixed(t === 6 ? 3 : 2));
      bar(`t${t}`, (v - lo) / (hi - lo));
    }
    const diet = d[L.iTraits + 4];
    set("dietword", diet < 0.33 ? "herbivore" : diet < 0.66 ? "omnivore" : "carnivore");
    this.drawBrain(core, d);
  }

  private buildCreature(core: Core, follow: boolean): void {
    this.mode = "creature";
    this.root.replaceChildren();
    this.vals.clear();
    this.bars.clear();
    const h2 = el("h2");
    const sw = el("span", "swatch");
    this.vals.set("swatch", sw);
    const idEl = el("span", undefined, "#");
    this.vals.set("id", idEl);
    const word = el("span", undefined, "");
    word.style.color = "var(--muted)";
    word.style.fontWeight = "400";
    word.style.marginLeft = "6px";
    this.vals.set("dietword", word);
    h2.append(sw, idEl, word);
    this.root.append(h2);

    const kv = el("div", "kv");
    const add = (k: string, label: string) => {
      kv.append(el("span", undefined, label));
      const v = el("span", undefined, "–");
      this.vals.set(k, v);
      kv.append(v);
    };
    add("gen", "generation");
    add("meals", "meals");
    this.root.append(kv);

    const addBar = (k: string, label: string) => {
      const row = el("div");
      const top = el("div", "kv");
      top.append(el("span", undefined, label));
      const v = el("span", undefined, "–");
      this.vals.set(k, v);
      top.append(v);
      const b = el("div", "bar");
      const fill = el("i");
      b.append(fill);
      this.bars.set(k, fill);
      row.append(top, b);
      this.root.append(row);
    };
    addBar("energy", "energy");
    addBar("age", "age");

    const row = el("div", "row");
    const lbl = el("label", "chk");
    const box = document.createElement("input");
    box.type = "checkbox";
    box.checked = follow;
    box.addEventListener("change", () => this.onFollowChange?.(box.checked));
    lbl.append(box, document.createTextNode("follow with camera"));
    row.append(lbl);
    this.root.append(row);
    this.followBox = box;

    this.root.append(el("h3", undefined, "Genome"));
    for (let t = 0; t < core.layout.nTraits; t++) {
      const tr = el("div", "trait");
      tr.append(el("span", "n", core.traitNames[t] ?? `t${t}`));
      const b = el("div", "bar");
      const fill = el("i");
      if (t === 3) fill.style.background = "linear-gradient(90deg, hsl(0 70% 60%), hsl(120 70% 60%), hsl(240 70% 60%), hsl(360 70% 60%))";
      b.append(fill);
      this.bars.set(`t${t}`, fill);
      const v = el("span", "v", "–");
      this.vals.set(`t${t}`, v);
      tr.append(b, v);
      this.root.append(tr);
    }

    this.root.append(el("h3", undefined, "Brain, live"));
    const canvas = document.createElement("canvas");
    canvas.id = "brain";
    this.root.append(canvas);
    this.brain = canvas;
    this.root.append(
      el("p", undefined, "Left: what it senses. Middle: hidden layer. Right: what it does. Teal links excite, orange inhibit; thicker is stronger. Node brightness is the current activation."),
    );
  }

  private drawBrain(core: Core, d: Float32Array): void {
    const canvas = this.brain;
    if (!canvas) return;
    const dpr = window.devicePixelRatio || 1;
    const w = canvas.clientWidth;
    const h = canvas.clientHeight;
    if (w === 0 || h === 0) return;
    if (canvas.width !== Math.floor(w * dpr) || canvas.height !== Math.floor(h * dpr)) {
      canvas.width = Math.floor(w * dpr);
      canvas.height = Math.floor(h * dpr);
    }
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);
    const L = core.layout;
    const xIn = 78;
    const xHid = w / 2 + 10;
    const xOut = w - 54;
    const yAt = (i: number, n: number) => 14 + ((i + 0.5) / n) * (h - 28);
    const wIn = (hid: number, inp: number) => d[L.iWeights + hid * L.nIn + inp];
    const wOut = (out: number, hid: number) => d[L.iWeights + L.nIn * L.nHid + out * (L.nHid + 1) + hid];

    const link = (x1: number, y1: number, x2: number, y2: number, wgt: number) => {
      const a = Math.min(1, Math.abs(wgt) / 2.5);
      if (a < 0.06) return;
      ctx.strokeStyle = wgt > 0 ? `rgba(90,209,179,${a * 0.9})` : `rgba(240,140,90,${a * 0.9})`;
      ctx.lineWidth = 0.4 + a * 2.2;
      ctx.beginPath();
      ctx.moveTo(x1, y1);
      ctx.lineTo(x2, y2);
      ctx.stroke();
    };
    for (let hid = 0; hid < L.nHid; hid++) {
      for (let inp = 0; inp < L.nIn; inp++) link(xIn, yAt(inp, L.nIn), xHid, yAt(hid, L.nHid), wIn(hid, inp));
      for (let out = 0; out < L.nOut; out++) link(xHid, yAt(hid, L.nHid), xOut, yAt(out, L.nOut), wOut(out, hid));
    }
    const node = (x: number, y: number, act: number, r: number) => {
      const a = Math.min(1, Math.abs(act));
      ctx.fillStyle = act >= 0 ? `rgba(214,222,232,${0.15 + a * 0.85})` : `rgba(240,140,90,${0.15 + a * 0.85})`;
      ctx.beginPath();
      ctx.arc(x, y, r, 0, Math.PI * 2);
      ctx.fill();
      ctx.strokeStyle = "#2a3744";
      ctx.lineWidth = 1;
      ctx.stroke();
    };
    ctx.font = "10px ui-sans-serif, system-ui";
    ctx.textBaseline = "middle";
    for (let i = 0; i < L.nIn; i++) {
      node(xIn, yAt(i, L.nIn), d[L.iInputs + i], 4.5);
      ctx.fillStyle = "#7f8ea0";
      ctx.textAlign = "right";
      ctx.fillText(core.inputNames[i] ?? "", xIn - 9, yAt(i, L.nIn));
    }
    for (let i = 0; i < L.nHid; i++) node(xHid, yAt(i, L.nHid), d[L.iHidden + i], 5);
    for (let i = 0; i < L.nOut; i++) {
      node(xOut, yAt(i, L.nOut), d[L.iOutputs + i], 5.5);
      ctx.fillStyle = "#7f8ea0";
      ctx.textAlign = "left";
      ctx.fillText(core.outputNames[i] ?? "", xOut + 10, yAt(i, L.nOut));
    }
  }

  reset(): void {
    this.mode = null;
  }
}
