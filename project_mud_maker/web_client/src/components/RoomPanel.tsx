import { useCallback, useEffect, useState } from 'react';
import type { Room, PlacedEntity } from '../types/world';
import { contentApi } from '../api/client';
import type { ContentItem } from '../types/content';

const DIRECTIONS = ['north', 'south', 'east', 'west', 'up', 'down'] as const;

interface RoomPanelProps {
  room: Room;
  allRooms: Room[];
  collections: string[];
  onChange: (room: Room) => void;
  onDelete: () => void;
}

export function RoomPanel({ room, allRooms, collections, onChange, onDelete }: RoomPanelProps) {
  const [contentItems, setContentItems] = useState<Record<string, ContentItem[]>>({});

  // Load content items for available collections
  useEffect(() => {
    const load = async () => {
      const result: Record<string, ContentItem[]> = {};
      for (const col of collections) {
        try {
          result[col] = await contentApi.listItems(col);
        } catch {
          // skip
        }
      }
      setContentItems(result);
    };
    load();
  }, [collections]);

  const update = useCallback(
    (patch: Partial<Room>) => {
      onChange({ ...room, ...patch });
    },
    [room, onChange],
  );

  // Exit management
  const addExit = () => {
    const dir = prompt(`Direction? (${DIRECTIONS.join(', ')})`);
    if (!dir || !DIRECTIONS.includes(dir as typeof DIRECTIONS[number])) return;
    if (room.exits[dir]) {
      alert(`Exit "${dir}" already exists`);
      return;
    }
    const targets = allRooms.filter((r) => r.id !== room.id);
    if (targets.length === 0) {
      alert('No other rooms to connect to');
      return;
    }
    const targetId = prompt(
      `Target room?\n${targets.map((r) => `  ${r.id} (${r.name})`).join('\n')}`,
    );
    if (!targetId || !targets.some((r) => r.id === targetId)) return;

    update({ exits: { ...room.exits, [dir]: targetId } });
  };

  const removeExit = (dir: string) => {
    const exits = { ...room.exits };
    delete exits[dir];
    update({ exits });
  };

  // Entity management
  const addEntity = () => {
    const type = prompt('Entity type? (npc, item)');
    if (!type || !['npc', 'item'].includes(type)) return;

    const collection = type === 'npc' ? 'monsters' : 'items';
    const items = contentItems[collection] || [];

    let contentId: string | null;
    if (items.length > 0) {
      contentId = prompt(
        `Content ID?\n${items.map((i) => `  ${i.id}`).join('\n')}`,
      );
    } else {
      contentId = prompt('Content ID:');
    }
    if (!contentId) return;

    const entity: PlacedEntity = { type, content_id: contentId };
    update({ entities: [...room.entities, entity] });
  };

  const removeEntity = (index: number) => {
    update({ entities: room.entities.filter((_, i) => i !== index) });
  };

  return (
    <div className="p-4 space-y-5">
      {/* Header */}
      <div className="flex items-center justify-between">
        <h3 className="text-lg font-bold text-gray-100">{room.name || room.id}</h3>
        <button
          onClick={onDelete}
          className="text-xs px-2 py-1 bg-red-700 hover:bg-red-600 rounded"
        >
          Delete
        </button>
      </div>

      {/* Basic info */}
      <div className="space-y-3">
        <div>
          <label className="block text-xs text-gray-400 mb-1">ID</label>
          <input
            type="text"
            value={room.id}
            disabled
            className="w-full bg-gray-700/50 text-gray-500 border border-gray-600 rounded px-2 py-1.5 text-sm"
          />
        </div>
        <div>
          <label className="block text-xs text-gray-400 mb-1">Name</label>
          <input
            type="text"
            value={room.name}
            onChange={(e) => update({ name: e.target.value })}
            className="w-full bg-gray-700 border border-gray-600 rounded px-2 py-1.5 text-sm"
          />
        </div>
        <div>
          <label className="block text-xs text-gray-400 mb-1">Description</label>
          <textarea
            value={room.description}
            onChange={(e) => update({ description: e.target.value })}
            rows={3}
            className="w-full bg-gray-700 border border-gray-600 rounded px-2 py-1.5 text-sm"
          />
        </div>
      </div>

      {/* Exits */}
      <div>
        <div className="flex items-center justify-between mb-2">
          <label className="text-xs text-gray-400 font-medium">Exits</label>
          <button onClick={addExit} className="text-xs text-blue-400 hover:text-blue-300">
            + Add
          </button>
        </div>
        {Object.keys(room.exits).length === 0 ? (
          <p className="text-xs text-gray-600">No exits</p>
        ) : (
          <div className="space-y-1">
            {Object.entries(room.exits).map(([dir, target]) => {
              const targetRoom = allRooms.find((r) => r.id === target);
              return (
                <div
                  key={dir}
                  className="flex items-center justify-between bg-gray-700/50 rounded px-2 py-1.5 text-sm"
                >
                  <span>
                    <span className="text-green-400 font-medium">{dir}</span>
                    {' → '}
                    <span className="text-gray-300">{targetRoom?.name || target}</span>
                  </span>
                  <button
                    onClick={() => removeExit(dir)}
                    className="text-gray-500 hover:text-red-400 text-xs"
                  >
                    x
                  </button>
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* Entities */}
      <div>
        <div className="flex items-center justify-between mb-2">
          <label className="text-xs text-gray-400 font-medium">Entities</label>
          <button onClick={addEntity} className="text-xs text-blue-400 hover:text-blue-300">
            + Add
          </button>
        </div>
        {room.entities.length === 0 ? (
          <p className="text-xs text-gray-600">No entities</p>
        ) : (
          <div className="space-y-1">
            {room.entities.map((ent, i) => (
              <div
                key={i}
                className="flex items-center justify-between bg-gray-700/50 rounded px-2 py-1.5 text-sm"
              >
                <span>
                  <span
                    className={
                      ent.type === 'npc' ? 'text-red-400' : 'text-yellow-400'
                    }
                  >
                    [{ent.type}]
                  </span>{' '}
                  <span className="text-gray-300">{ent.content_id}</span>
                </span>
                <button
                  onClick={() => removeEntity(i)}
                  className="text-gray-500 hover:text-red-400 text-xs"
                >
                  x
                </button>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
