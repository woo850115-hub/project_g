import { useCallback, useEffect, useMemo, useState } from 'react';
import {
  ReactFlow,
  ReactFlowProvider,
  Background,
  Controls,
  MiniMap,
  type Node,
  type Edge,
  type OnNodesChange,
  type OnEdgesChange,
  type OnConnect,
  type Connection,
  applyEdgeChanges,
  MarkerType,
} from '@xyflow/react';
import { worldApi, contentApi } from '../api/client';
import type { Room, WorldData, Zone } from '../types/world';
import { RoomNode } from '../components/RoomNode';
import { RoomPanel } from '../components/RoomPanel';
import { AddRoomDialog, AddConnectedRoomDialog, ConnectDialog, ConfirmDialog } from '../components/Modal';
import { Tooltip } from '../components/Tooltip';

const DIRECTIONS = ['north', 'south', 'east', 'west', 'up', 'down'] as const;
const OPPOSITE: Record<string, string> = {
  north: 'south', south: 'north',
  east: 'west', west: 'east',
  up: 'down', down: 'up',
};

const DIR_LABEL_SHORT: Record<string, string> = {
  north: '북', south: '남',
  east: '동', west: '서',
  up: '상', down: '하',
};

const DIR_SOURCE_HANDLE: Record<string, string> = {
  north: 's-top', south: 's-bottom',
  east: 's-right', west: 's-left',
  up: 's-top', down: 's-bottom',
};
const DIR_TARGET_HANDLE: Record<string, string> = {
  north: 't-bottom', south: 't-top',
  east: 't-left', west: 't-right',
  up: 't-bottom', down: 't-top',
};

const DIR_OFFSET: Record<string, { x: number; y: number }> = {
  north: { x: 0, y: -200 }, south: { x: 0, y: 200 },
  east: { x: 250, y: 0 },   west: { x: -250, y: 0 },
  up: { x: 80, y: -150 },   down: { x: 80, y: 150 },
};

export function MapEditor() {
  return (
    <ReactFlowProvider>
      <MapEditorInner />
    </ReactFlowProvider>
  );
}

function MapEditorInner() {
  const [world, setWorld] = useState<WorldData>({ rooms: [] });
  const [selectedRoomId, setSelectedRoomId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [luaPreview, setLuaPreview] = useState<string | null>(null);
  const [collections, setCollections] = useState<string[]>([]);
  const [filterZoneId, setFilterZoneId] = useState<string | null>(null);

  // Zone management dialog
  const [zoneDialogOpen, setZoneDialogOpen] = useState(false);
  const [newZoneName, setNewZoneName] = useState('');
  const [newZoneColor, setNewZoneColor] = useState('#3b82f6');

  // Dialog states
  const [addRoomOpen, setAddRoomOpen] = useState(false);
  const [connectDialog, setConnectDialog] = useState<{
    open: boolean;
    source: string;
    target: string;
  }>({ open: false, source: '', target: '' });
  const [deleteDialog, setDeleteDialog] = useState<{
    open: boolean;
    roomId: string;
    roomName: string;
  }>({ open: false, roomId: '', roomName: '' });

  // Connected room dialog
  const [connectedRoomDialog, setConnectedRoomDialog] = useState<{
    open: boolean;
    direction: string;
    parentRoomId: string;
    parentRoomName: string;
  }>({ open: false, direction: '', parentRoomId: '', parentRoomName: '' });


  const nodeTypes = useMemo(() => ({ room: RoomNode }), []);

  const zones = world.zones || [];

  const zoneMap = useMemo(() => {
    const map: Record<string, Zone> = {};
    for (const z of zones) map[z.id] = z;
    return map;
  }, [zones]);

  // Load world data
  const loadWorld = useCallback(async () => {
    try {
      const data = await worldApi.get();
      setWorld(data);
    } catch (e) {
      setError(e instanceof Error ? e.message : '월드 데이터를 불러올 수 없습니다');
    }
  }, []);

  useEffect(() => {
    loadWorld();
    contentApi.listCollections().then(setCollections).catch(() => {});
  }, [loadWorld]);

  // Filter rooms by zone
  const visibleRooms = filterZoneId
    ? world.rooms.filter((r) => r.zone_id === filterZoneId)
    : world.rooms;

  // Track measured node dimensions so React Flow can show them (visibility: visible)
  const [nodeMeasured, setNodeMeasured] = useState<
    Record<string, { width: number; height: number }>
  >({});

  // Convert world rooms to React Flow nodes (memoized)
  const nodes: Node[] = useMemo(() =>
    visibleRooms.map((room) => {
      const zone = room.zone_id ? zoneMap[room.zone_id] : undefined;
      const measured = nodeMeasured[room.id];
      return {
        id: room.id,
        type: 'room',
        position: { x: room.position.x, y: room.position.y },
        data: {
          label: room.name || room.id,
          entityCount: room.entities.length,
          exitCount: Object.keys(room.exits).length,
          zoneName: zone?.name,
          zoneColor: zone?.color,
        },
        selected: room.id === selectedRoomId,
        ...(measured ? { measured } : {}),
      };
    }),
    [visibleRooms, zoneMap, nodeMeasured, selectedRoomId],
  );

  // Convert exits to edges (memoized)
  const edges: Edge[] = useMemo(() => {
    const result: Edge[] = [];
    const edgeSet = new Set<string>();
    const visibleIds = new Set(visibleRooms.map((r) => r.id));
    for (const room of visibleRooms) {
      for (const [dir, targetId] of Object.entries(room.exits)) {
        if (!visibleIds.has(targetId)) continue;
        const edgeId = [room.id, targetId].sort().join('--');
        if (edgeSet.has(edgeId)) continue;
        edgeSet.add(edgeId);

        const targetRoom = world.rooms.find((r) => r.id === targetId);
        const reverseDir = targetRoom
          ? Object.entries(targetRoom.exits).find(([, t]) => t === room.id)?.[0]
          : undefined;

        const dirKo = DIR_LABEL_SHORT[dir] || dir;
        const reverseDirKo = reverseDir ? (DIR_LABEL_SHORT[reverseDir] || reverseDir) : undefined;
        const label = reverseDirKo ? `${dirKo} / ${reverseDirKo}` : dirKo;

        result.push({
          id: `${room.id}-${dir}-${targetId}`,
          source: room.id,
          target: targetId,
          sourceHandle: DIR_SOURCE_HANDLE[dir] || 's-top',
          targetHandle: DIR_TARGET_HANDLE[dir] || 't-bottom',
          label,
          style: { stroke: '#6b7280' },
          labelStyle: { fill: '#9ca3af', fontSize: 11 },
          markerEnd: { type: MarkerType.ArrowClosed, color: '#6b7280' },
        });
      }
    }
    return result;
  }, [visibleRooms, world.rooms]);

  // Handle node changes (dimensions, position, selection)
  const onNodesChange: OnNodesChange = useCallback(
    (changes) => {
      let hasDimensionChange = false;

      for (const change of changes) {
        if (change.type === 'dimensions' && change.dimensions) {
          hasDimensionChange = true;
        }
        if (change.type === 'position' && change.position) {
          setWorld((prev) => {
            const updated = { ...prev, rooms: [...prev.rooms] };
            const idx = updated.rooms.findIndex((r) => r.id === change.id);
            if (idx >= 0) {
              updated.rooms[idx] = {
                ...updated.rooms[idx],
                position: { x: change.position!.x, y: change.position!.y },
              };
            }
            return updated;
          });
        }
        if (change.type === 'select' && change.selected) {
          setSelectedRoomId(change.id);
        }
      }

      if (hasDimensionChange) {
        setNodeMeasured((prev) => {
          const next = { ...prev };
          for (const change of changes) {
            if (change.type === 'dimensions' && change.dimensions) {
              next[change.id] = {
                width: change.dimensions.width,
                height: change.dimensions.height,
              };
            }
          }
          return next;
        });
      }
    },
    [],
  );

  const onEdgesChange: OnEdgesChange = useCallback(
    (changes) => {
      applyEdgeChanges(changes, edges);
    },
    [edges],
  );

  // Handle new edge connection — open dialog
  const onConnect: OnConnect = useCallback(
    (connection: Connection) => {
      if (!connection.source || !connection.target) return;
      setConnectDialog({
        open: true,
        source: connection.source,
        target: connection.target,
      });
    },
    [],
  );

  const handleConnect = (direction: string, bidirectional: boolean) => {
    const { source, target } = connectDialog;
    setWorld((prev) => {
      const updated = { ...prev, rooms: prev.rooms.map((r) => ({ ...r })) };
      const src = updated.rooms.find((r) => r.id === source);
      const tgt = updated.rooms.find((r) => r.id === target);
      if (src) {
        src.exits = { ...src.exits, [direction]: target };
      }
      if (bidirectional && tgt && OPPOSITE[direction]) {
        tgt.exits = { ...tgt.exits, [OPPOSITE[direction]]: source };
      }
      return updated;
    });
    setConnectDialog({ open: false, source: '', target: '' });
  };

  // Add new room — dialog callback
  const handleAddRoom = (id: string, name: string, zoneId?: string) => {
    const pos = { x: Math.random() * 400, y: Math.random() * 400 };
    setWorld((prev) => ({
      ...prev,
      rooms: [
        ...prev.rooms,
        {
          id,
          name,
          description: '',
          position: pos,
          exits: {},
          entities: [],
          zone_id: zoneId || filterZoneId || undefined,
        },
      ],
    }));
    setSelectedRoomId(id);
    setAddRoomOpen(false);
  };

  // Add connected room — from compass
  const handleAddConnectedRoom = (direction: string) => {
    if (!selectedRoom) return;
    setConnectedRoomDialog({
      open: true,
      direction,
      parentRoomId: selectedRoom.id,
      parentRoomName: selectedRoom.name || selectedRoom.id,
    });
  };

  const handleConnectedRoomSubmit = (id: string, name: string) => {
    const { direction, parentRoomId } = connectedRoomDialog;
    const parentRoom = world.rooms.find((r) => r.id === parentRoomId);
    if (!parentRoom) return;

    const offset = DIR_OFFSET[direction] || { x: 200, y: 0 };
    const newPos = {
      x: parentRoom.position.x + offset.x,
      y: parentRoom.position.y + offset.y,
    };

    setWorld((prev) => {
      const rooms = prev.rooms.map((r) => {
        if (r.id === parentRoomId) {
          return { ...r, exits: { ...r.exits, [direction]: id } };
        }
        return r;
      });
      const reverseDir = OPPOSITE[direction];
      const newRoom: Room = {
        id,
        name,
        description: '',
        position: newPos,
        exits: reverseDir ? { [reverseDir]: parentRoomId } : {},
        entities: [],
        zone_id: parentRoom.zone_id,
      };
      return { ...prev, rooms: [...rooms, newRoom] };
    });

    setSelectedRoomId(id);
    setConnectedRoomDialog({ open: false, direction: '', parentRoomId: '', parentRoomName: '' });
  };

  // Delete room
  const requestDeleteRoom = (roomId: string) => {
    const room = world.rooms.find((r) => r.id === roomId);
    setDeleteDialog({ open: true, roomId, roomName: room?.name || roomId });
  };

  const handleDeleteRoom = () => {
    const { roomId } = deleteDialog;
    setWorld((prev) => {
      const rooms = prev.rooms
        .filter((r) => r.id !== roomId)
        .map((r) => ({
          ...r,
          exits: Object.fromEntries(
            Object.entries(r.exits).filter(([, target]) => target !== roomId),
          ),
        }));
      return { ...prev, rooms };
    });
    if (selectedRoomId === roomId) setSelectedRoomId(null);
    setDeleteDialog({ open: false, roomId: '', roomName: '' });
  };

  // Update selected room
  const updateRoom = (updated: Room) => {
    setWorld((prev) => ({
      ...prev,
      rooms: prev.rooms.map((r) => (r.id === updated.id ? updated : r)),
    }));
  };

  // Zone management
  const addZone = () => {
    if (!newZoneName.trim()) return;
    const id = newZoneName.trim().toLowerCase().replace(/\s+/g, '_');
    if (zones.some((z) => z.id === id)) {
      setError('이미 같은 ID의 존이 있습니다');
      return;
    }
    setWorld((prev) => ({
      ...prev,
      zones: [...(prev.zones || []), { id, name: newZoneName.trim(), color: newZoneColor }],
    }));
    setNewZoneName('');
    setNewZoneColor('#3b82f6');
    setZoneDialogOpen(false);
  };

  const deleteZone = (zoneId: string) => {
    setWorld((prev) => ({
      ...prev,
      zones: (prev.zones || []).filter((z) => z.id !== zoneId),
      rooms: prev.rooms.map((r) =>
        r.zone_id === zoneId ? { ...r, zone_id: undefined } : r,
      ),
    }));
    if (filterZoneId === zoneId) setFilterZoneId(null);
  };

  const editZone = (zoneId: string, name: string, color: string) => {
    setWorld((prev) => ({
      ...prev,
      zones: (prev.zones || []).map((z) =>
        z.id === zoneId ? { ...z, name, color } : z,
      ),
    }));
  };

  // Pane click — deselect room
  const handlePaneClick = useCallback(() => {
    setSelectedRoomId(null);
  }, []);

  // Save world
  const saveWorld = async () => {
    setSaving(true);
    try {
      await worldApi.save(world);
    } catch (e) {
      setError(e instanceof Error ? e.message : '저장에 실패했습니다');
    } finally {
      setSaving(false);
    }
  };

  // Generate Lua
  const generateLua = async () => {
    try {
      await worldApi.save(world);
      const result = await worldApi.generate();
      setLuaPreview(result.preview);
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Lua 생성에 실패했습니다');
    }
  };

  const selectedRoom = world.rooms.find((r) => r.id === selectedRoomId) || null;

  return (
    <div className="flex h-full">
      {/* Error toast */}
      {error && (
        <div className="fixed top-4 right-4 bg-red-600 text-white px-4 py-2 rounded shadow-lg z-50">
          {error}
          <button className="ml-2 font-bold" onClick={() => setError(null)}>x</button>
        </div>
      )}

      {/* Dialogs */}
      <AddRoomDialog
        open={addRoomOpen}
        existingIds={world.rooms.map((r) => r.id)}
        zones={zones}
        defaultZoneId={filterZoneId || undefined}
        onSubmit={handleAddRoom}
        onCancel={() => setAddRoomOpen(false)}
      />
      <AddConnectedRoomDialog
        open={connectedRoomDialog.open}
        direction={connectedRoomDialog.direction}
        parentRoomName={connectedRoomDialog.parentRoomName}
        existingIds={world.rooms.map((r) => r.id)}
        onSubmit={handleConnectedRoomSubmit}
        onCancel={() => setConnectedRoomDialog({ open: false, direction: '', parentRoomId: '', parentRoomName: '' })}
      />
      <ConnectDialog
        open={connectDialog.open}
        sourceId={connectDialog.source}
        targetId={connectDialog.target}
        directions={[...DIRECTIONS]}
        onSubmit={handleConnect}
        onCancel={() => setConnectDialog({ open: false, source: '', target: '' })}
      />
      <ConfirmDialog
        open={deleteDialog.open}
        title="방 삭제"
        message={`"${deleteDialog.roomName}" 방을 삭제하시겠습니까? 이 방으로의 모든 출구도 함께 제거됩니다.`}
        confirmLabel="삭제"
        onConfirm={handleDeleteRoom}
        onCancel={() => setDeleteDialog({ open: false, roomId: '', roomName: '' })}
      />

      {/* Zone add dialog */}
      {zoneDialogOpen && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="bg-gray-800 border border-gray-600 rounded-lg p-5 w-80 space-y-4">
            <h3 className="text-sm font-bold">존 추가</h3>
            <div>
              <label className="block text-xs text-gray-400 mb-1">이름</label>
              <input
                type="text"
                value={newZoneName}
                onChange={(e) => setNewZoneName(e.target.value)}
                placeholder="예: 마을, 숲, 던전"
                className="w-full bg-gray-700 border border-gray-600 rounded px-2 py-1.5 text-sm"
                autoFocus
                onKeyDown={(e) => e.key === 'Enter' && addZone()}
              />
            </div>
            <div>
              <label className="block text-xs text-gray-400 mb-1">색상</label>
              <div className="flex items-center gap-2">
                <input
                  type="color"
                  value={newZoneColor}
                  onChange={(e) => setNewZoneColor(e.target.value)}
                  className="w-8 h-8 rounded border border-gray-600 cursor-pointer"
                />
                <span className="text-xs text-gray-400">{newZoneColor}</span>
              </div>
            </div>
            <div className="flex justify-end gap-2">
              <button
                onClick={() => { setZoneDialogOpen(false); setNewZoneName(''); }}
                className="px-3 py-1.5 text-xs bg-gray-600 hover:bg-gray-500 rounded"
              >
                취소
              </button>
              <button
                onClick={addZone}
                disabled={!newZoneName.trim()}
                className="px-3 py-1.5 text-xs bg-blue-600 hover:bg-blue-500 disabled:opacity-50 rounded"
              >
                추가
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Canvas area */}
      <div className="flex-1 flex flex-col">
        {/* Toolbar */}
        <div className="flex items-center gap-3 px-4 py-2 border-b border-gray-700 bg-gray-800">
          <Tooltip text="맵에 새로운 방을 추가합니다">
            <button
              onClick={() => setAddRoomOpen(true)}
              className="px-3 py-1 text-xs bg-blue-600 hover:bg-blue-500 rounded"
            >
              + 방 추가
            </button>
          </Tooltip>
          <Tooltip text="현재 맵 데이터를 서버에 저장합니다">
            <button
              onClick={saveWorld}
              disabled={saving}
              className="px-3 py-1 text-xs bg-green-700 hover:bg-green-600 disabled:opacity-50 rounded"
            >
              {saving ? '저장 중...' : '저장'}
            </button>
          </Tooltip>
          <Tooltip text="맵 데이터를 기반으로 Lua 스크립트를 자동 생성합니다">
            <button
              onClick={generateLua}
              className="px-3 py-1 text-xs bg-purple-700 hover:bg-purple-600 rounded"
            >
              Lua 생성
            </button>
          </Tooltip>

          {/* Zone filter */}
          <div className="flex items-center gap-1 ml-4 border-l border-gray-600 pl-4">
            <span className="text-xs text-gray-500">존:</span>
            <button
              onClick={() => setFilterZoneId(null)}
              className={`px-2 py-0.5 text-xs rounded ${
                filterZoneId === null
                  ? 'bg-gray-600 text-white'
                  : 'text-gray-400 hover:text-gray-200'
              }`}
            >
              전체
            </button>
            {zones.map((z) => (
              <button
                key={z.id}
                onClick={() => setFilterZoneId(z.id)}
                className={`px-2 py-0.5 text-xs rounded flex items-center gap-1 ${
                  filterZoneId === z.id
                    ? 'bg-gray-600 text-white'
                    : 'text-gray-400 hover:text-gray-200'
                }`}
              >
                <span
                  className="w-2 h-2 rounded-full inline-block"
                  style={{ backgroundColor: z.color }}
                />
                {z.name}
              </button>
            ))}
            <Tooltip text="새로운 존을 추가합니다">
              <button
                onClick={() => setZoneDialogOpen(true)}
                className="px-1.5 py-0.5 text-xs text-gray-500 hover:text-blue-400"
              >
                +
              </button>
            </Tooltip>
          </div>

          <span className="text-xs text-gray-500 ml-auto">
            {visibleRooms.length}개 방{filterZoneId ? ` (${zones.find((z) => z.id === filterZoneId)?.name})` : ''} | 노드를 드래그하여 출구를 연결하세요
          </span>
        </div>

        {/* React Flow canvas */}
        <div style={{ flex: '1 1 0%', overflow: 'hidden' }}>
          <ReactFlow
            nodes={nodes}
            edges={edges}
            onNodesChange={onNodesChange}
            onEdgesChange={onEdgesChange}
            onConnect={onConnect}
            onPaneClick={handlePaneClick}
            nodeTypes={nodeTypes}
            fitView
            colorMode="dark"
            style={{ width: '100%', height: '100%' }}
            defaultEdgeOptions={{
              style: { stroke: '#6b7280' },
              markerEnd: { type: MarkerType.ArrowClosed, color: '#6b7280' },
            }}
          >
            <Background color="#374151" gap={20} />
            <Controls />
            <MiniMap
              nodeColor={(node) => {
                const color = node.data?.zoneColor;
                return typeof color === 'string' ? color : '#3b82f6';
              }}
              maskColor="rgba(0,0,0,0.7)"
              style={{ background: '#1f2937' }}
            />
          </ReactFlow>
        </div>

        {/* Lua preview panel */}
        {luaPreview && (
          <div className="border-t border-gray-700 bg-gray-900 max-h-64 overflow-y-auto">
            <div className="flex items-center justify-between px-3 py-1.5 bg-gray-800 border-b border-gray-700">
              <span className="text-xs text-gray-400">생성된 Lua 미리보기</span>
              <button
                onClick={() => setLuaPreview(null)}
                className="text-xs text-gray-500 hover:text-gray-300"
              >
                닫기
              </button>
            </div>
            <pre className="p-3 text-xs font-mono text-green-300 whitespace-pre-wrap">
              {luaPreview}
            </pre>
          </div>
        )}
      </div>

      {/* Right panel: room properties */}
      <div className="w-80 border-l border-gray-700 bg-gray-800 overflow-y-auto">
        {selectedRoom ? (
          <RoomPanel
            room={selectedRoom}
            allRooms={world.rooms}
            zones={zones}
            collections={collections}
            onChange={updateRoom}
            onDelete={() => requestDeleteRoom(selectedRoom.id)}
            onDeleteZone={deleteZone}
            onEditZone={editZone}
            onAddConnectedRoom={handleAddConnectedRoom}
          />
        ) : (
          <div className="flex items-center justify-center h-full text-gray-500 text-sm">
            편집할 방을 선택하세요
          </div>
        )}
      </div>
    </div>
  );
}
