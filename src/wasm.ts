// Loads the wasm-pack output and wraps the bits of linear memory the renderer
// reads. Views are re-created on every access because WebAssembly memory can
// grow, which detaches any Float32Array made over the old buffer.
import init, { Sim } from "../pkg/tidepool_core.js";
import type { InitOutput } from "../pkg/tidepool_core.js";

export { Sim };

export interface Layout {
  creatureStride: number;
  plantStride: number;
  nIn: number;
  nHid: number;
  nOut: number;
  nW: number;
  nTraits: number;
  nFields: number;
  hueBins: number;
  sampleStride: number;
  inspectLen: number;
  iTraits: number;
  iInputs: number;
  iHidden: number;
  iOutputs: number;
  iWeights: number;
}

// Stats sample fields, mirroring core/src/stats.rs.
export const F = {
  tick: 0,
  pop: 1,
  plants: 2,
  herb: 3,
  omni: 4,
  carn: 5,
  births: 6,
  deaths: 7,
  approach: 8,
  meanSize: 9,
  meanSpeed: 10,
  meanSense: 11,
  meanDiet: 12,
  nutrients: 13,
  generation: 14,
} as const;

// Creature view columns, mirroring CREATURE_STRIDE in core/src/lib.rs.
export const C = { x: 0, y: 1, heading: 2, radius: 3, hue: 4, energy: 5, diet: 6, slot: 7 } as const;

// Inspect row scalar offsets, mirroring the I_* constants.
export const I = { id: 0, generation: 1, age: 2, maxAge: 3, energy: 4, maxEnergy: 5, meals: 6, radius: 7 } as const;

export class Core {
  private constructor(
    readonly wasm: InitOutput,
    readonly layout: Layout,
    readonly inputNames: string[],
    readonly outputNames: string[],
    readonly traitNames: string[],
  ) {}

  static async load(): Promise<Core> {
    const wasm = await init();
    const l = Array.from(Sim.layout());
    const layout: Layout = {
      creatureStride: l[0],
      plantStride: l[1],
      nIn: l[2],
      nHid: l[3],
      nOut: l[4],
      nW: l[5],
      nTraits: l[6],
      nFields: l[7],
      hueBins: l[8],
      sampleStride: l[9],
      inspectLen: l[10],
      iTraits: l[11],
      iInputs: l[12],
      iHidden: l[13],
      iOutputs: l[14],
      iWeights: l[15],
    };
    return new Core(wasm, layout, Sim.input_names().split("|"), Sim.output_names().split("|"), Sim.trait_names().split("|"));
  }

  /** A Float32Array over `len` floats at byte offset `ptr` in WASM memory. */
  f32(ptr: number, len: number): Float32Array {
    return new Float32Array(this.wasm.memory.buffer, ptr, len);
  }

  creatures(sim: Sim): Float32Array {
    return this.f32(sim.creatures_ptr(), sim.creatures_len() * this.layout.creatureStride);
  }

  plants(sim: Sim): Float32Array {
    return this.f32(sim.plants_ptr(), sim.plants_len() * this.layout.plantStride);
  }

  nutrients(sim: Sim): Float32Array {
    return this.f32(sim.nutrients_ptr(), sim.grid_cols() * sim.grid_rows());
  }

  /** The whole history ring buffer; use `historyRow` to index it in time order. */
  history(sim: Sim): Float32Array {
    return this.f32(sim.history_ptr(), sim.history_cap() * this.layout.sampleStride);
  }

  /** Byte-free index of the i-th oldest retained sample row. */
  historyRowStart(sim: Sim, i: number): number {
    const cap = sim.history_cap();
    const len = sim.history_len();
    const head = sim.history_head();
    return ((head + cap - len + i) % cap) * this.layout.sampleStride;
  }
}
