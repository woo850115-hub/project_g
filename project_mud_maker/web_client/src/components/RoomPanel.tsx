import { useCallback, useEffect, useMemo, useState } from 'react';
import type { Room, PlacedEntity, Zone } from '../types/world';
import { contentApi, attributeSchemaApi } from '../api/client';
import type { ContentItem } from '../types/content';
import type { AttributeSchema } from '../types/attribute_schema';
import { AddExitDialog, AddEntityDialog } from './Modal';
import { Tooltip } from './Tooltip';

const DIRECTIONS = ['north', 'south', 'east', 'west', 'up', 'down'] as const;

// Static override fields by entity type
const BASE_OVERRIDE_FIELDS: Record<string, { key: string; label: string; type: 'number' | 'string' }[]> = {
  npc: [
    { key: 'name', label: '이름', type: 'string' },
    { key: 'hp', label: 'HP', type: 'number' },
    { key: 'attack', label: '공격력', type: 'number' },
    { key: 'defense', label: '방어력', type: 'number' },
    { key: 'level', label: '레벨', type: 'number' },
    { key: 'gold', label: '골드', type: 'number' },
  ],
  item: [
    { key: 'name', label: '이름', type: 'string' },
  ],
};

// Map entity type to content collection for schema lookup
const ENTITY_TYPE_TO_COLLECTION: Record<string, string> = {
  npc: 'monsters',
  item: 'items',
};

function schemaToOverrideType(vt: string): 'number' | 'string' {
  return vt === 'number' || vt === 'range' ? 'number' : 'string';
}

const DIR_LABELS: Record<string, string> = {
  north: '북쪽', south: '남쪽',
  east: '동쪽', west: '서쪽',
  up: '위', down: '아래',
};

interface RoomPanelProps {
  room: Room;
  allRooms: Room[];
  zones: Zone[];
  collections: string[];
  onChange: (room: Room) => void;
  onDelete: () => void;
  onDeleteZone: (zoneId: string) => void;
  onEditZone: (zoneId: string, name: string, color: string) => void;
  onAddConnectedRoom: (direction: string) => void;
}

export function RoomPanel({ room, allRooms, zones, collections, onChange, onDelete, onDeleteZone, onEditZone, onAddConnectedRoom }: RoomPanelProps) {
  const [contentItems, setContentItems] = useState<Record<string, ContentItem[]>>({});
  const [exitDialogOpen, setExitDialogOpen] = useState(false);
  const [entityDialogOpen, setEntityDialogOpen] = useState(false);
  const [editingZoneId, setEditingZoneId] = useState<string | null>(null);
  const [editZoneName, setEditZoneName] = useState('');
  const [editZoneColor, setEditZoneColor] = useState('');
  const [expandedEntity, setExpandedEntity] = useState<number | null>(null);
  const [attrSchemas, setAttrSchemas] = useState<AttributeSchema[]>([]);

  // Load content items for available collections + attribute schemas
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
      try {
        const schemas = await attributeSchemaApi.list();
        setAttrSchemas(schemas);
      } catch { /* ignore */ }
    };
    load();
  }, [collections]);

  // Build dynamic override fields per entity type using schemas
  const dynamicOverrideFields = useMemo(() => {
    const result: Record<string, { key: string; label: string; type: 'number' | 'string' }[]> = {};
    for (const [entType, baseFields] of Object.entries(BASE_OVERRIDE_FIELDS)) {
      const collection = ENTITY_TYPE_TO_COLLECTION[entType];
      const schemaFields = attrSchemas
        .filter((s) => s.applies_to.length === 0 || (collection && s.applies_to.includes(collection)))
        .filter((s) => !baseFields.some((f) => f.key === s.id))
        .map((s) => ({
          key: s.id,
          label: s.label,
          type: schemaToOverrideType(s.value_type),
        }));
      result[entType] = [...baseFields, ...schemaFields];
    }
    return result;
  }, [attrSchemas]);

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
    if (expandedEntity === index) setExpandedEntity(null);
  };

  // Update an override field on an entity
  const updateEntityOverride = (index: number, key: string, value: unknown) => {
    const entities = [...room.entities];
    const ent = { ...entities[index] };
    const overrides = { ...(ent.overrides || {}) };
    if (value === '' || value === undefined) {
      delete overrides[key];
    } else {
      overrides[key] = value;
    }
    ent.overrides = Object.keys(overrides).length > 0 ? overrides : undefined;
    entities[index] = ent;
    update({ entities });
  };

  // Get content default for a placed entity field
  const getContentDefault = (ent: PlacedEntity, key: string): string => {
    const collection = ent.type === 'npc' ? 'monsters' : 'items';
    const items = contentItems[collection] || [];
    const item = items.find((i) => i.id === ent.content_id);
    if (!item) return '';
    const val = (item as Record<string, unknown>)[key];
    if (val === undefined || val === null) return '';
    return String(val);
  };

  const currentZone = zones.find((z) => z.id === room.zone_id);

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

        {/* Zone selector */}
        <div>
          <label className="block text-xs text-gray-400 mb-1">존</label>
          <div className="flex items-center gap-2">
            <select
              value={room.zone_id || ''}
              onChange={(e) => update({ zone_id: e.target.value || undefined })}
              className="flex-1 bg-gray-700 border border-gray-600 rounded px-2 py-1.5 text-sm"
            >
              <option value="">없음</option>
              {zones.map((z) => (
                <option key={z.id} value={z.id}>
                  {z.name}
                </option>
              ))}
            </select>
            {currentZone && (
              <span
                className="w-4 h-4 rounded-full border border-gray-500 flex-shrink-0"
                style={{ backgroundColor: currentZone.color }}
              />
            )}
          </div>
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

      {/* Quick-connect compass */}
      {availableDirections.length > 0 && (
        <div>
          <label className="text-xs text-gray-400 font-medium mb-2 block">방향별 방 추가</label>
          <div className="grid grid-cols-3 gap-1 w-36 mx-auto">
            <div />
            {availableDirections.includes('north') ? (
              <button
                onClick={() => onAddConnectedRoom('north')}
                className="px-2 py-1 text-xs bg-gray-700 hover:bg-blue-600 rounded text-center"
              >
                북
              </button>
            ) : <div className="px-2 py-1 text-xs text-gray-700 text-center">-</div>}
            {availableDirections.includes('up') ? (
              <button
                onClick={() => onAddConnectedRoom('up')}
                className="px-2 py-1 text-xs bg-gray-700 hover:bg-blue-600 rounded text-center"
              >
                위
              </button>
            ) : <div className="px-2 py-1 text-xs text-gray-700 text-center">-</div>}
            {availableDirections.includes('west') ? (
              <button
                onClick={() => onAddConnectedRoom('west')}
                className="px-2 py-1 text-xs bg-gray-700 hover:bg-blue-600 rounded text-center"
              >
                서
              </button>
            ) : <div className="px-2 py-1 text-xs text-gray-700 text-center">-</div>}
            <div className="px-2 py-1 text-xs text-gray-500 text-center font-bold">+</div>
            {availableDirections.includes('east') ? (
              <button
                onClick={() => onAddConnectedRoom('east')}
                className="px-2 py-1 text-xs bg-gray-700 hover:bg-blue-600 rounded text-center"
              >
                동
              </button>
            ) : <div className="px-2 py-1 text-xs text-gray-700 text-center">-</div>}
            <div />
            {availableDirections.includes('south') ? (
              <button
                onClick={() => onAddConnectedRoom('south')}
                className="px-2 py-1 text-xs bg-gray-700 hover:bg-blue-600 rounded text-center"
              >
                남
              </button>
            ) : <div className="px-2 py-1 text-xs text-gray-700 text-center">-</div>}
            {availableDirections.includes('down') ? (
              <button
                onClick={() => onAddConnectedRoom('down')}
                className="px-2 py-1 text-xs bg-gray-700 hover:bg-blue-600 rounded text-center"
              >
                아래
              </button>
            ) : <div className="px-2 py-1 text-xs text-gray-700 text-center">-</div>}
          </div>
        </div>
      )}

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
            {room.entities.map((ent, i) => {
              const isExpanded = expandedEntity === i;
              const overrideFields = dynamicOverrideFields[ent.type] || [];
              const hasOverrides = ent.overrides && Object.keys(ent.overrides).length > 0;
              return (
                <div key={i} className="bg-gray-700/50 rounded">
                  <div
                    className="flex items-center justify-between px-2 py-1.5 text-sm cursor-pointer hover:bg-gray-700/80"
                    onClick={() => setExpandedEntity(isExpanded ? null : i)}
                  >
                    <span className="flex items-center gap-1">
                      <span className="text-gray-500 text-xs">{isExpanded ? '\u25BC' : '\u25B6'}</span>
                      <span
                        className={
                          ent.type === 'npc' ? 'text-red-400' : 'text-yellow-400'
                        }
                      >
                        [{ent.type}]
                      </span>{' '}
                      <span className="text-gray-300">{ent.content_id}</span>
                      {hasOverrides && (
                        <span className="text-[9px] text-blue-400 bg-blue-400/10 px-1 rounded">
                          override
                        </span>
                      )}
                    </span>
                    <button
                      onClick={(e) => { e.stopPropagation(); removeEntity(i); }}
                      className="text-gray-500 hover:text-red-400 text-xs"
                    >
                      x
                    </button>
                  </div>
                  {isExpanded && overrideFields.length > 0 && (
                    <div className="px-3 pb-2 pt-1 border-t border-gray-600/50 space-y-1.5">
                      <p className="text-[10px] text-gray-500">
                        속성 오버라이드 (비워두면 콘텐츠 기본값 사용)
                      </p>
                      {overrideFields.map((field) => {
                        const overrideVal = ent.overrides?.[field.key];
                        const contentDefault = getContentDefault(ent, field.key);
                        return (
                          <div key={field.key} className="flex items-center gap-2">
                            <label className="w-14 text-[10px] text-gray-400 text-right shrink-0">
                              {field.label}
                            </label>
                            <input
                              type={field.type === 'number' ? 'number' : 'text'}
                              value={overrideVal !== undefined ? String(overrideVal) : ''}
                              placeholder={contentDefault || '기본값'}
                              onChange={(e) => {
                                const val = field.type === 'number'
                                  ? (e.target.value === '' ? '' : Number(e.target.value))
                                  : e.target.value;
                                updateEntityOverride(i, field.key, val);
                              }}
                              className="flex-1 bg-gray-600 border border-gray-500 rounded px-1.5 py-0.5 text-xs placeholder-gray-500"
                            />
                          </div>
                        );
                      })}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* Zone management (at bottom) */}
      {zones.length > 0 && (
        <div className="border-t border-gray-700 pt-4">
          <label className="text-xs text-gray-400 font-medium">존 관리</label>
          <div className="space-y-1 mt-2">
            {zones.map((z) => (
              <div
                key={z.id}
                className="flex items-center justify-between bg-gray-700/50 rounded px-2 py-1.5 text-sm"
              >
                {editingZoneId === z.id ? (
                  <div className="flex items-center gap-2 flex-1 mr-2">
                    <input
                      type="color"
                      value={editZoneColor}
                      onChange={(e) => setEditZoneColor(e.target.value)}
                      className="w-6 h-6 rounded border border-gray-500 cursor-pointer"
                    />
                    <input
                      type="text"
                      value={editZoneName}
                      onChange={(e) => setEditZoneName(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter') {
                          onEditZone(z.id, editZoneName, editZoneColor);
                          setEditingZoneId(null);
                        }
                        if (e.key === 'Escape') setEditingZoneId(null);
                      }}
                      className="flex-1 bg-gray-600 border border-gray-500 rounded px-1.5 py-0.5 text-xs"
                      autoFocus
                    />
                    <button
                      onClick={() => {
                        onEditZone(z.id, editZoneName, editZoneColor);
                        setEditingZoneId(null);
                      }}
                      className="text-xs text-green-400 hover:text-green-300"
                    >
                      OK
                    </button>
                  </div>
                ) : (
                  <span className="flex items-center gap-2">
                    <span
                      className="w-3 h-3 rounded-full border border-gray-500"
                      style={{ backgroundColor: z.color }}
                    />
                    <span className="text-gray-300">{z.name}</span>
                    <span className="text-gray-600 text-xs">
                      ({allRooms.filter((r) => r.zone_id === z.id).length})
                    </span>
                  </span>
                )}
                {editingZoneId !== z.id && (
                  <div className="flex items-center gap-1">
                    <button
                      onClick={() => {
                        setEditingZoneId(z.id);
                        setEditZoneName(z.name);
                        setEditZoneColor(z.color);
                      }}
                      className="text-gray-500 hover:text-blue-400 text-xs"
                    >
                      edit
                    </button>
                    <button
                      onClick={() => onDeleteZone(z.id)}
                      className="text-gray-500 hover:text-red-400 text-xs"
                    >
                      x
                    </button>
                  </div>
                )}
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
