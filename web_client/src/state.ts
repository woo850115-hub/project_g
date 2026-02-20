import type {
  GridConfig,
  EntityWire,
  EntityMovedWire,
} from "./protocol";

export interface EntityState {
  id: number;
  x: number;
  y: number;
  name: string;
  isSelf: boolean;
  // Rendering interpolation positions (float)
  renderX: number;
  renderY: number;
}

export class GameState {
  connected = false;
  sessionId = 0;
  selfEntityId = 0;
  tick = 0;
  gridConfig: GridConfig | null = null;
  entities: Map<number, EntityState> = new Map();

  reset(): void {
    this.connected = false;
    this.sessionId = 0;
    this.selfEntityId = 0;
    this.tick = 0;
    this.gridConfig = null;
    this.entities.clear();
  }

  applyEntered(entries: EntityWire[]): void {
    for (const e of entries) {
      this.entities.set(e.id, {
        id: e.id,
        x: e.x,
        y: e.y,
        name: e.name ?? `Entity ${e.id}`,
        isSelf: e.is_self,
        renderX: e.x,
        renderY: e.y,
      });
    }
  }

  applyMoved(moves: EntityMovedWire[]): void {
    for (const m of moves) {
      const ent = this.entities.get(m.id);
      if (ent) {
        ent.x = m.x;
        ent.y = m.y;
      }
    }
  }

  applyLeft(ids: number[]): void {
    for (const id of ids) {
      this.entities.delete(id);
    }
  }
}
