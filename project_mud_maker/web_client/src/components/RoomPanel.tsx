import { useCallback, useEffect, useState } from 'react';
import type { Room, PlacedEntity } from '../types/world';
import { contentApi } from '../api/client';
import type { ContentItem } from '../types/content';
import { AddExitDialog, AddEntityDialog } from './Modal';
import { Tooltip } from './Tooltip';

const DIRECTIONS = ['north', 'south', 'east', 'west', 'up', 'down'] as const;

const DIR_LABELS: Record<string, string> = {
  north: '\uBD81\uCABD', south: '\uB0A8\uCABD',
  east: '\uB3D9\uCABD', west: '\uC11C\uCABD',
  up: '\uC704', down: '\uC544\uB798',
};

interface RoomPanelProps {
  room: Room;
  allRooms: Room[];
  collections: string[];
  onChange: (room: Room) => void;
  onDelete: () => void;
}

export function RoomPanel({ room, allRooms, collections, onChange, onDelete }: RoomPanelProps) {
  const [contentItems, setContentItems] = useState<Record<string, ContentItem[]>>({});
  const [exitDialogOpen, setExitDialogOpen] = useState(false);
  const [entityDialogOpen, setEntityDialogOpen] = useState(false);

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
  const usedDirections = Object.keys(room.exits);
  const availableDirections = DIRECTIONS.filter((d) => !usedDirections.includes(d));
  const targetRooms = allRooms
    .filter((r) => r.id !== room.id)
    .map((r) => ({ id: r.id, name: r.name || r.id }));

  const handleAddExit = (direction: string, targetId: string) => {
    update({ exits: { ...room.exits, [direction]: targetId } });
    setExitDialogOpen(false);
  };

  const removeExit = (dir: string) => {
    const exits = { ...room.exits };
    delete exits[dir];
    update({ exits });
  };

  // Entity management
  const handleAddEntity = (type: string, contentId: string) => {
    const entity: PlacedEntity = { type, content_id: contentId };
    update({ entities: [...room.entities, entity] });
    setEntityDialogOpen(false);
  };

  const removeEntity = (index: number) => {
    update({ entities: room.entities.filter((_, i) => i !== index) });
  };

  return (
    <div className="p-4 space-y-5">
      {/* Dialogs */}
      <AddExitDialog
        open={exitDialogOpen}
        availableDirections={[...availableDirections]}
        targetRooms={targetRooms}
        onSubmit={handleAddExit}
        onCancel={() => setExitDialogOpen(false)}
      />
      <AddEntityDialog
        open={entityDialogOpen}
        contentItems={contentItems}
        onSubmit={handleAddEntity}
        onCancel={() => setEntityDialogOpen(false)}
      />

      {/* Header */}
      <div className="flex items-center justify-between">
        <h3 className="text-lg font-bold text-gray-100">{room.name || room.id}</h3>
        <button
          onClick={onDelete}
          className="text-xs px-2 py-1 bg-red-700 hover:bg-red-600 rounded"
        >
          삭제
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
          <label className="block text-xs text-gray-400 mb-1">이름</label>
          <input
            type="text"
            value={room.name}
            onChange={(e) => update({ name: e.target.value })}
            className="w-full bg-gray-700 border border-gray-600 rounded px-2 py-1.5 text-sm"
          />
        </div>
        <div>
          <label className="block text-xs text-gray-400 mb-1">설명</label>
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
          <label className="text-xs text-gray-400 font-medium">출구</label>
          <Tooltip text="다른 방으로 이동할 수 있는 출구를 추가합니다">
            <button
              onClick={() => setExitDialogOpen(true)}
              className="text-xs text-blue-400 hover:text-blue-300"
            >
              + 추가
            </button>
          </Tooltip>
        </div>
        {Object.keys(room.exits).length === 0 ? (
          <p className="text-xs text-gray-600">출구 없음</p>
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
                    <span className="text-green-400 font-medium">{DIR_LABELS[dir] || dir}</span>
                    {' \u2192 '}
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
          <label className="text-xs text-gray-400 font-medium">엔티티</label>
          <Tooltip text="이 방에 NPC나 아이템을 배치합니다">
            <button
              onClick={() => setEntityDialogOpen(true)}
              className="text-xs text-blue-400 hover:text-blue-300"
            >
              + 추가
            </button>
          </Tooltip>
        </div>
        {room.entities.length === 0 ? (
          <p className="text-xs text-gray-600">엔티티 없음</p>
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
