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

  const nodeTypes = useMemo(() => ({ room: RoomNode }), []);

  // Load world data
  const loadWorld = useCallback(async () => {
    try {
      const data = await worldApi.get();
      setWorld(data);
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to load world');
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

      // Find reverse direction
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
      // Apply visual changes
      const updatedNodes = applyNodeChanges(changes, nodes);

      // Update world data with new positions
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

      // Handle selection
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

  // Handle new edge connection (create exit)
  const onConnect: OnConnect = useCallback(
    (connection: Connection) => {
      if (!connection.source || !connection.target) return;
      const dir = prompt(`Direction from source to target?\n(${DIRECTIONS.join(', ')})`);
      if (!dir || !DIRECTIONS.includes(dir as typeof DIRECTIONS[number])) return;

      const bidir = confirm('Create bidirectional exit?');

      setWorld((prev) => {
        const updated = { ...prev, rooms: prev.rooms.map((r) => ({ ...r })) };
        const src = updated.rooms.find((r) => r.id === connection.source);
        const tgt = updated.rooms.find((r) => r.id === connection.target);
        if (src) {
          src.exits = { ...src.exits, [dir]: connection.target! };
        }
        if (bidir && tgt && OPPOSITE[dir]) {
          tgt.exits = { ...tgt.exits, [OPPOSITE[dir]]: connection.source! };
        }
        return updated;
      });
    },
    [],
  );

  // Add new room
  const addRoom = () => {
    const id = prompt('Room ID (e.g. tavern):');
    if (!id) return;
    if (world.rooms.some((r) => r.id === id)) {
      setError('Room ID already exists');
      return;
    }

    const name = prompt('Room name:') || id;

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
  };

  // Delete room
  const deleteRoom = (roomId: string) => {
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
      setError(e instanceof Error ? e.message : 'Save failed');
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
      setError(e instanceof Error ? e.message : 'Generate failed');
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

      {/* Canvas area */}
      <div className="flex-1 flex flex-col">
        {/* Toolbar */}
        <div className="flex items-center gap-3 px-4 py-2 border-b border-gray-700 bg-gray-800">
          <button
            onClick={addRoom}
            className="px-3 py-1 text-xs bg-blue-600 hover:bg-blue-500 rounded"
          >
            + Add Room
          </button>
          <button
            onClick={saveWorld}
            disabled={saving}
            className="px-3 py-1 text-xs bg-green-700 hover:bg-green-600 disabled:opacity-50 rounded"
          >
            {saving ? 'Saving...' : 'Save'}
          </button>
          <button
            onClick={generateLua}
            className="px-3 py-1 text-xs bg-purple-700 hover:bg-purple-600 rounded"
          >
            Generate Lua
          </button>
          <span className="text-xs text-gray-500 ml-auto">
            {world.rooms.length} rooms | Drag nodes to connect exits
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
              <span className="text-xs text-gray-400">Generated Lua Preview</span>
              <button
                onClick={() => setLuaPreview(null)}
                className="text-xs text-gray-500 hover:text-gray-300"
              >
                Close
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
            onDelete={() => deleteRoom(selectedRoom.id)}
          />
        ) : (
          <div className="flex items-center justify-center h-full text-gray-500 text-sm">
            Select a room to edit
          </div>
        )}
      </div>
    </div>
  );
}
