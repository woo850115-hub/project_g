export interface ShopItem {
  item_id: string;
  price: number;
}

export interface Shop {
  id: string;
  name: string;
  npc_name: string;
  room_name: string;
  items: ShopItem[];
  sell_rate: number;
}
