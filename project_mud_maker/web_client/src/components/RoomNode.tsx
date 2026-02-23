import { Handle, Position } from '@xyflow/react';

interface RoomNodeData {
  label: string;
  entityCount: number;
  exitCount: number;
  [key: string]: unknown;
}

export function RoomNode({ data, selected }: { data: RoomNodeData; selected?: boolean }) {
  return (
    <div
      className={`px-4 py-3 rounded-lg border-2 shadow-lg min-w-[140px] text-center transition-colors ${
        selected
          ? 'bg-blue-900/60 border-blue-400'
          : 'bg-gray-800 border-gray-600 hover:border-gray-400'
      }`}
    >
      <Handle type="target" position={Position.Top} className="!bg-gray-500 !w-3 !h-3" />
      <Handle type="target" position={Position.Left} className="!bg-gray-500 !w-3 !h-3" />

      <div className="text-sm font-bold text-gray-100">{data.label}</div>

      <div className="flex items-center justify-center gap-3 mt-1 text-[10px] text-gray-400">
        {data.exitCount > 0 && <span>{data.exitCount} exits</span>}
        {data.entityCount > 0 && <span>{data.entityCount} entities</span>}
      </div>

      <Handle type="source" position={Position.Bottom} className="!bg-blue-500 !w-3 !h-3" />
      <Handle type="source" position={Position.Right} className="!bg-blue-500 !w-3 !h-3" />
    </div>
  );
}
