export interface DialogueChoice {
  text: string;
  next: string | null;
  action?: string;
}

export interface DialogueNode {
  id: string;
  text: string;
  choices: DialogueChoice[];
}

export interface Dialogue {
  id: string;
  npc_name: string;
  nodes: DialogueNode[];
}
