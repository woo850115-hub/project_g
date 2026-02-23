export type TriggerCondition =
  | { type: 'enter_room'; room_id: string }
  | { type: 'command'; command: string }
  | { type: 'tick_interval'; interval: number }
  | { type: 'entity_death'; content_id: string }
  | { type: 'on_connect' };

export type TriggerAction =
  | { type: 'send_message'; target: string; text: string }
  | { type: 'spawn_entity'; entity_type: string; content_id: string; room_id: string }
  | { type: 'teleport'; room_id: string }
  | { type: 'set_component'; target: string; component: string; value: unknown }
  | { type: 'despawn_trigger_entity' }
  | { type: 'give_item'; content_id: string }
  | { type: 'heal'; target: string; mode: string; amount: number };

export interface Trigger {
  id: string;
  name: string;
  enabled: boolean;
  condition: TriggerCondition;
  actions: TriggerAction[];
}
