import { App } from "./ui/app.js";
import { Core } from "./wasm.js";

function seedFromHash(): number {
  const m = /seed=(\d+)/.exec(location.hash);
  if (m) return Number(m[1]) >>> 0;
  return (Math.random() * 0xffffffff) >>> 0;
}

async function main(): Promise<void> {
  const core = await Core.load();
  const seed = seedFromHash();
  const sexual = /(^|[#&])sexual(?:=(?:1|true))?(&|$)/.test(location.hash);
  const tm = /(^|[#&])t=(\d+)/.exec(location.hash);
  const targetTick = tm ? Number(tm[2]) : 0;
  history.replaceState(null, "", `#seed=${seed}${sexual ? "&sexual" : ""}${targetTick > 0 ? `&t=${targetTick}` : ""}`);
  new App(core, seed, sexual, targetTick);
}

main().catch((err: unknown) => {
  const app = document.getElementById("app");
  if (app) app.textContent = `Tidepool failed to start: ${err instanceof Error ? err.message : String(err)}`;
  console.error(err);
});
