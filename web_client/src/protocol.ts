// Client → Server messages

export interface ConnectMessage {
  type: "connect";
  name: string;
}

export interface MoveMessage {
  type: "move";
  dx: number;
  dy: number;
}

export interface ActionMessage {
  type: "action";
  name: string;
  args?: string;
}

export interface PingMessage {
  type: "ping";
}

export type ClientMessage =
  | ConnectMessage
  | MoveMessage
  | ActionMessage
  | PingMessage;

// Server → Client messages

export interface GridConfig {
  width: number;
  height: number;
  origin_x: number;
  origin_y: number;
}

export interface EntityWire {
  id: number;
  x: number;
  y: number;
  name?: string;
  is_self: boolean;
}

export interface EntityMovedWire {
  id: number;
  x: number;
  y: number;
}

export interface WelcomeMessage {
  type: "welcome";
  session_id: number;
  entity_id: number;
  tick: number;
  grid_config: GridConfig;
}

export interface StateDeltaMessage {
  type: "state_delta";
  tick: number;
  entered?: EntityWire[];
  moved?: EntityMovedWire[];
  left?: number[];
}

export interface ErrorMessage {
  type: "error";
  message: string;
}

export interface PongMessage {
  type: "pong";
}

export type ServerMessage =
  | WelcomeMessage
  | StateDeltaMessage
  | ErrorMessage
  | PongMessage;
