import { memo } from 'react';
import { Handle, Position } from '@xyflow/react';

interface RoomNodeData {
  label: string;
  entityCount: number;
  exitCount: number;
  zoneName?: string;
  zoneColor?: string;
  [key: string]: unknown;
}

export const RoomNode = memo(function RoomNode({ data, selected }: { data: RoomNodeData; selected?: boolean }) {
  const borderColor = selected
    ? '#60a5fa'
    : data.zoneColor || '#4b5563';

  return (
    <div
      className={`px-4 py-3 rounded-lg border-2 shadow-lg min-w-[140px] text-center transition-colors ${
        selected ? 'bg-blue-900/60' : 'bg-gray-800 hover:brightness-110'
      }`}
      style={{ borderColor }}
    >
      <Handle type="target" position={Position.Top} id="t-top" className="!bg-gray-500 !w-2.5 !h-2.5" />
      <Handle type="source" position={Position.Top} id="s-top" className="!bg-gray-500 !w-2.5 !h-2.5" />
      <Handle type="target" position={Position.Bottom} id="t-bottom" className="!bg-gray-500 !w-2.5 !h-2.5" />
      <Handle type="source" position={Position.Bottom} id="s-bottom" className="!bg-gray-500 !w-2.5 !h-2.5" />
      <Handle type="target" position={Position.Left} id="t-left" className="!bg-gray-500 !w-2.5 !h-2.5" />
      <Handle type="source" position={Position.Left} id="s-left" className="!bg-gray-500 !w-2.5 !h-2.5" />
      <Handle type="target" position={Position.Right} id="t-right" className="!bg-gray-500 !w-2.5 !h-2.5" />
      <Handle type="source" position={Position.Right} id="s-right" className="!bg-gray-500 !w-2.5 !h-2.5" />

      <div className="text-sm font-bold text-gray-100">{data.label}</div>

      <div className="flex items-center justify-center gap-3 mt-1 text-[10px] text-gray-400">
        {data.zoneName && (
          <span style={{ color: data.zoneColor }}>{data.zoneName}</span>
        )}
        {data.exitCount > 0 && <span>{data.exitCount} 출구</span>}
        {data.entityCount > 0 && <span>{data.entityCount} 엔티티</span>}
      </div>
    </div>
  );
});
