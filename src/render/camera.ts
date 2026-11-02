// A camera over a toroidal world. Everything is positioned relative to the
// camera centre using the shortest wrapped offset, so the view scrolls
// seamlessly across the seam and each entity is drawn at most once.
export class Camera {
  cx: number;
  cy: number;
  /** Pixels per world unit. */
  zoom = 1;
  viewW = 1;
  viewH = 1;

  constructor(readonly worldW: number, readonly worldH: number) {
    this.cx = worldW / 2;
    this.cy = worldH / 2;
  }

  resize(viewW: number, viewH: number): void {
    this.viewW = viewW;
    this.viewH = viewH;
    this.clampZoom();
  }

  /** Smallest zoom at which the viewport still fits inside the world. */
  minZoom(): number {
    return Math.max(this.viewW / this.worldW, this.viewH / this.worldH);
  }

  fit(): void {
    this.zoom = this.minZoom();
    this.cx = this.worldW / 2;
    this.cy = this.worldH / 2;
  }

  private clampZoom(): void {
    this.zoom = Math.min(Math.max(this.zoom, this.minZoom()), 12);
  }

  wrapDx(dx: number): number {
    const w = this.worldW;
    if (dx > w / 2) return dx - w;
    if (dx < -w / 2) return dx + w;
    return dx;
  }

  wrapDy(dy: number): number {
    const h = this.worldH;
    if (dy > h / 2) return dy - h;
    if (dy < -h / 2) return dy + h;
    return dy;
  }

  screenX(x: number): number {
    return this.viewW / 2 + this.wrapDx(x - this.cx) * this.zoom;
  }

  screenY(y: number): number {
    return this.viewH / 2 + this.wrapDy(y - this.cy) * this.zoom;
  }

  worldX(sx: number): number {
    return this.wrap(this.cx + (sx - this.viewW / 2) / this.zoom, this.worldW);
  }

  worldY(sy: number): number {
    return this.wrap(this.cy + (sy - this.viewH / 2) / this.zoom, this.worldH);
  }

  pan(dxPix: number, dyPix: number): void {
    this.cx = this.wrap(this.cx - dxPix / this.zoom, this.worldW);
    this.cy = this.wrap(this.cy - dyPix / this.zoom, this.worldH);
  }

  /** Zoom by `factor` keeping the world point under (sx, sy) fixed. */
  zoomAt(sx: number, sy: number, factor: number): void {
    const wx = this.worldX(sx);
    const wy = this.worldY(sy);
    this.zoom *= factor;
    this.clampZoom();
    // Move the centre so wx,wy lands back under sx,sy.
    this.cx = this.wrap(wx - (sx - this.viewW / 2) / this.zoom, this.worldW);
    this.cy = this.wrap(wy - (sy - this.viewH / 2) / this.zoom, this.worldH);
  }

  centerOn(x: number, y: number): void {
    this.cx = x;
    this.cy = y;
  }

  private wrap(v: number, span: number): number {
    return ((v % span) + span) % span;
  }
}
