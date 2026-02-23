export interface Position {
  x: number;
  y: number;
}

export interface PlacedEntity {
  type: string; // "npc" | "item"
  content_id: string;
  overrides?: Record<string, unknown>;
}

export interface Zone {
  id: string;
  name: string;
  color: string; // hex color for map display
}

export interface Room {
  id: string;
  name: string;
  description: string;
  position: Position;
  exits: Record<string, string>;
  entities: PlacedEntity[];
  zone_id?: string;
}

export interface WorldData {
  zones?: Zone[];
  rooms: Room[];
}

export interface GenerateResult {
  ok: boolean;
  path: string;
  preview: string;
}
