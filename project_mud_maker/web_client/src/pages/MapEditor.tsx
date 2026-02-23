import { useCallback, useEffect, useMemo, useState } from 'react';
import {
  ReactFlow,
  Background,
  Controls,
  MiniMap,
  type Node,
  type Edge,
  type OnNodesChange,
  type OnEdgesChange,
  type OnConnect,
  type Connection,
  applyNodeChanges,
  applyEdgeChanges,
  MarkerType,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import { worldApi, contentApi } from '../api/client';
import type { Room, WorldData } from '../types/world';
import { RoomNode } from '../components/RoomNode';
import { RoomPanel } from '../components/RoomPanel';
import { AddRoomDialog, ConnectDialog, ConfirmDialog } from '../components/Modal';
import { Tooltip } from '../components/Tooltip';

const DIRECTIONS = ['north', 'south', 'east', 'west', 'up', 'down'] as const;
const OPPOSITE: Record<string, string> = {
  north: 'south', south: 'north',
  east: 'west', west: 'east',
  up: 'down', down: 'up',
};

export function MapEditor() {
  const [world, setWorld] = useState<WorldData>({ rooms: [] });
  const [selectedRoomId, setSelectedRoomId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [luaPreview, setLuaPreview] = useState<string | null>(null);
  const [collections, setCollections] = useState<string[]>([]);

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

  const nodeTypes = useMemo(() => ({ room: RoomNode }), []);

  // Load world data
  const loadWorld = useCallback(async () => {
    try {
      const data = await worldApi.get();
      setWorld(data);
    } catch (e) {
      setError(e instanceof Error ? e.message : '\uC6D4\uB4DC \uB370\uC774\uD130\uB97C \uBD88\uB7EC\uC62C \uC218 \uC5C6\uC2B5\uB2C8\uB2E4');
    }
  }, []);

  useEffect(() => {
    loadWorld();
    contentApi.listCollections().then(setCollections).catch(() => {});
  }, [loadWorld]);

  // Convert world rooms to React Flow nodes
  const nodes: Node[] = world.rooms.map((room) => ({
    id: room.id,
    type: 'room',
    position: { x: room.position.x, y: room.position.y },
    data: {
      label: room.name || room.id,
      entityCount: room.entities.length,
      exitCount: Object.keys(room.exits).length,
    },
    selected: room.id === selectedRoomId,
  }));

  // Convert exits to edges
  const edges: Edge[] = [];
  const edgeSet = new Set<string>();
  for (const room of world.rooms) {
    for (const [dir, targetId] of Object.entries(room.exits)) {
      const edgeId = [room.id, targetId].sort().join('--');
      if (edgeSet.has(edgeId)) continue;
      edgeSet.add(edgeId);

      const targetRoom = world.rooms.find((r) => r.id === targetId);
      const reverseDir = targetRoom
        ? Object.entries(targetRoom.exits).find(([, t]) => t === room.id)?.[0]
        : undefined;

      const label = reverseDir ? `${dir} / ${reverseDir}` : dir;

      edges.push({
        id: `${room.id}-${dir}-${targetId}`,
        source: room.id,
        target: targetId,
        label,
        style: { stroke: '#6b7280' },
        labelStyle: { fill: '#9ca3af', fontSize: 11 },
        markerEnd: { type: MarkerType.ArrowClosed, color: '#6b7280' },
      });
    }
  }

  // Handle node position changes
  const onNodesChange: OnNodesChange = useCallback(
    (changes) => {
      const updatedNodes = applyNodeChanges(changes, nodes);

      setWorld((prev) => {
        const updated = { ...prev, rooms: [...prev.rooms] };
        for (const change of changes) {
          if (change.type === 'position' && change.position) {
            const idx = updated.rooms.findIndex((r) => r.id === change.id);
            if (idx >= 0) {
              updated.rooms[idx] = {
                ...updated.rooms[idx],
                position: { x: change.position.x, y: change.position.y },
              };
            }
          }
        }
        return updated;
      });

      for (const change of changes) {
        if (change.type === 'select' && change.selected) {
          setSelectedRoomId(change.id);
        }
      }

      return updatedNodes;
    },
    [nodes],
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
  const handleAddRoom = (id: string, name: string) => {
    setWorld((prev) => ({
      ...prev,
      rooms: [
        ...prev.rooms,
        {
          id,
          name,
          description: '',
          position: { x: Math.random() * 400, y: Math.random() * 400 },
          exits: {},
          entities: [],
        },
      ],
    }));
    setSelectedRoomId(id);
    setAddRoomOpen(false);
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

  // Save world
  const saveWorld = async () => {
    setSaving(true);
    try {
      await worldApi.save(world);
    } catch (e) {
      setError(e instanceof Error ? e.message : '\uC800\uC7A5\uC5D0 \uC2E4\uD328\uD588\uC2B5\uB2C8\uB2E4');
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
      setError(e instanceof Error ? e.message : 'Lua \uC0DD\uC131\uC5D0 \uC2E4\uD328\uD588\uC2B5\uB2C8\uB2E4');
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
        onSubmit={handleAddRoom}
        onCancel={() => setAddRoomOpen(false)}
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
          <span className="text-xs text-gray-500 ml-auto">
            {world.rooms.length}개 방 | 노드를 드래그하여 출구를 연결하세요
          </span>
        </div>

        {/* React Flow canvas */}
        <div className="flex-1">
          <ReactFlow
            nodes={nodes}
            edges={edges}
            onNodesChange={onNodesChange}
            onEdgesChange={onEdgesChange}
            onConnect={onConnect}
            nodeTypes={nodeTypes}
            fitView
            colorMode="dark"
            defaultEdgeOptions={{
              style: { stroke: '#6b7280' },
              markerEnd: { type: MarkerType.ArrowClosed, color: '#6b7280' },
            }}
          >
            <Background color="#374151" gap={20} />
            <Controls />
            <MiniMap
              nodeColor="#3b82f6"
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
            collections={collections}
            onChange={updateRoom}
            onDelete={() => requestDeleteRoom(selectedRoom.id)}
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
