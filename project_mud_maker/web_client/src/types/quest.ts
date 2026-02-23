export interface QuestObjective {
  type: 'kill' | 'collect' | 'visit' | 'talk';
  target: string;
  count: number;
}

export interface QuestRewards {
  gold: number;
  exp: number;
  items: string[];
}

export interface Quest {
  id: string;
  name: string;
  description: string;
  npc_name: string;
  auto_complete: boolean;
  objectives: QuestObjective[];
  rewards: QuestRewards;
}
