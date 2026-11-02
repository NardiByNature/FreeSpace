// Kept specimens: named genome codes in localStorage. A per-viewer
// convenience only; everything works without it.

export interface Specimen {
  name: string;
  code: string;
  hue: number;
  diet: number;
  generation: number;
  kept: number;
}

const KEY = "tidepool.specimens.v1";

export function loadSpecimens(): Specimen[] {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return [];
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter((s): s is Specimen => typeof s === "object" && s !== null && typeof (s as Specimen).code === "string");
  } catch {
    return [];
  }
}

export function saveSpecimens(list: Specimen[]): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(list.slice(0, 50)));
  } catch {
    // Storage unavailable (private mode, quota); the list simply does not persist.
  }
}

export function dietWord(diet: number): string {
  return diet < 0.33 ? "herbivore" : diet < 0.66 ? "omnivore" : "carnivore";
}
