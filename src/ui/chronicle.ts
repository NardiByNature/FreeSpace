// Turns Chronicle events from the core into sentences. Kinds mirror the
// EV_* constants in core/src/stats.rs.

export const EV = {
  firstHunters: 0,
  huntersGone: 1,
  boom: 2,
  crash: 3,
  lineageLost: 4,
  lineageDominant: 5,
  generation: 6,
  washedIn: 7,
  quiet: 8,
  speciation: 9,
} as const;

export interface EventRow {
  tick: number;
  kind: number;
  a: number;
  b: number;
}

export interface Described {
  text: string;
  /** Hue in [0,1) when the event is about a colour lineage. */
  hue?: number;
  /** Single-character mark for chart hairlines. */
  glyph: string;
}

const pct = (v: number) => `${Math.round(v * 100)}%`;
const n = (v: number) => Math.round(v).toLocaleString();

export function describe(e: EventRow): Described {
  switch (e.kind) {
    case EV.firstHunters:
      return { text: `First hunters. ${n(e.a)} carnivores roam the dish.`, glyph: "▲" };
    case EV.huntersGone:
      return { text: "The hunters are gone. Grazers have the dish to themselves.", glyph: "▽" };
    case EV.boom:
      return { text: `Boom. ${n(e.b)} became ${n(e.a)} in a thousand ticks.`, glyph: "+" };
    case EV.crash:
      return { text: `Crash. ${n(e.b)} fell to ${n(e.a)} in a thousand ticks.`, glyph: "–" };
    case EV.lineageLost:
      return { text: `A lineage died out. At its peak it was ${pct(e.b)} of all life.`, hue: e.a, glyph: "×" };
    case EV.lineageDominant:
      return { text: `One colour now dominates: ${pct(e.b)} of the dish.`, hue: e.a, glyph: "●" };
    case EV.generation:
      return { text: `Generation ${n(e.a)}.`, glyph: "g" };
    case EV.washedIn:
      return { text: `The tide brought ${n(e.a)} newcomers.`, glyph: "~" };
    case EV.quiet:
      return { text: "The dish went quiet.", glyph: "∅" };
    case EV.speciation:
      return { text: "Two lineages have stopped mixing.", hue: e.a, glyph: "⑂" };
    default:
      return { text: `Something happened (kind ${e.kind}).`, glyph: "?" };
  }
}

/** Parse the flat [tick, kind, a, b] rows the facade returns. */
export function parseEvents(flat: Float32Array): EventRow[] {
  const out: EventRow[] = [];
  for (let i = 0; i + 3 < flat.length; i += 4) {
    out.push({ tick: flat[i], kind: flat[i + 1], a: flat[i + 2], b: flat[i + 3] });
  }
  return out;
}
