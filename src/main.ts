// Pipeline smoke test: load the WASM module and show that it answers.
import init, { hello } from "../pkg/tidepool_core.js";

async function main(): Promise<void> {
  await init();
  const app = document.getElementById("app");
  if (!app) throw new Error("no #app");
  app.textContent = `tidepool core says ${hello(42)}`;
}

void main();
