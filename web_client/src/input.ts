import type { GameConnection } from "./ws";

const THROTTLE_MS = 100;

interface Direction {
  dx: number;
  dy: number;
}

const KEY_MAP: Record<string, Direction> = {
  w: { dx: 0, dy: -1 },
  a: { dx: -1, dy: 0 },
  s: { dx: 0, dy: 1 },
  d: { dx: 1, dy: 0 },
  arrowup: { dx: 0, dy: -1 },
  arrowleft: { dx: -1, dy: 0 },
  arrowdown: { dx: 0, dy: 1 },
  arrowright: { dx: 1, dy: 0 },
};

export class InputHandler {
  private pressed = new Set<string>();
  private lastSendTime = 0;
  private rafId = 0;
  private active = false;

  constructor(private connection: GameConnection) {}

  start(): void {
    if (this.active) return;
    this.active = true;

    window.addEventListener("keydown", this.onKeyDown);
    window.addEventListener("keyup", this.onKeyUp);
    this.poll();
  }

  stop(): void {
    this.active = false;
    this.pressed.clear();
    window.removeEventListener("keydown", this.onKeyDown);
    window.removeEventListener("keyup", this.onKeyUp);
    if (this.rafId) {
      cancelAnimationFrame(this.rafId);
      this.rafId = 0;
    }
  }

  private onKeyDown = (e: KeyboardEvent): void => {
    const key = e.key.toLowerCase();
    if (key in KEY_MAP) {
      e.preventDefault();
      this.pressed.add(key);
    }
  };

  private onKeyUp = (e: KeyboardEvent): void => {
    const key = e.key.toLowerCase();
    this.pressed.delete(key);
  };

  private poll = (): void => {
    if (!this.active) return;

    const now = performance.now();
    if (this.pressed.size > 0 && now - this.lastSendTime >= THROTTLE_MS) {
      // Use the most recently pressed direction key
      let dir: Direction | null = null;
      for (const key of this.pressed) {
        const d = KEY_MAP[key];
        if (d) dir = d;
      }
      if (dir) {
        this.connection.send({ type: "move", dx: dir.dx, dy: dir.dy });
        this.lastSendTime = now;
      }
    }

    this.rafId = requestAnimationFrame(this.poll);
  };
}
