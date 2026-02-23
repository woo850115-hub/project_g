export interface Position {
  x: number;
  y: number;
}

export interface PlacedEntity {
  type: string; // "npc" | "item"
  content_id: string;
  overrides?: Record<string, unknown>;
}

export interface Room {
  id: string;
  name: string;
  description: string;
  position: Position;
  exits: Record<string, string>;
  entities: PlacedEntity[];
}

export interface WorldData {
  rooms: Room[];
}

export interface GenerateResult {
  ok: boolean;
  path: string;
  preview: string;
}
