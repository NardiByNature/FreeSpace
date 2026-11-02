let timer = 0;

/** One transient message near the bottom of the canvas. */
export function toast(text: string, ms = 6000): void {
  const el = document.getElementById("toast");
  if (!el) return;
  el.textContent = text;
  el.hidden = false;
  el.classList.remove("fade");
  window.clearTimeout(timer);
  timer = window.setTimeout(() => {
    el.classList.add("fade");
    timer = window.setTimeout(() => {
      el.hidden = true;
    }, 700);
  }, ms);
}
