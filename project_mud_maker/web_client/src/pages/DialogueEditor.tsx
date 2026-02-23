import { useCallback, useEffect, useMemo, useState } from 'react';
import { contentApi, dialogueApi } from '../api/client';
import type { Dialogue, DialogueNode, DialogueChoice } from '../types/dialogue';
import { PromptDialog, ConfirmDialog } from '../components/Modal';

export function DialogueEditor() {
  const [dialogues, setDialogues] = useState<Dialogue[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [luaPreview, setLuaPreview] = useState<string | null>(null);

  const [createDialog, setCreateDialog] = useState(false);
  const [deleteDialog, setDeleteDialog] = useState(false);

  const loadDialogues = useCallback(async () => {
    try {
      const data = await contentApi.listItems('dialogues');
      setDialogues(data as unknown as Dialogue[]);
    } catch {
      setDialogues([]);
    }
  }, []);

  useEffect(() => {
    loadDialogues();
  }, [loadDialogues]);

  const selected = dialogues.find((d) => d.id === selectedId) || null;

  const updateDialogue = (updated: Dialogue) => {
    setDialogues((prev) =>
      prev.map((d) => (d.id === updated.id ? updated : d))
    );
  };

  const handleCreate = (name: string) => {
    setCreateDialog(false);
    const id = name.toLowerCase().replace(/[^a-z0-9]+/g, '_').replace(/^_|_$/g, '');
    if (dialogues.some((d) => d.id === id)) {
      setError(`대화 ID "${id}"이(가) 이미 존재합니다`);
      return;
    }
    const newDialogue: Dialogue = {
      id,
      npc_name: name,
      nodes: [
        { id: 'start', text: '', choices: [{ text: '', next: null }] },
      ],
    };
    setDialogues((prev) => [...prev, newDialogue]);
    setSelectedId(id);
  };

  const handleDelete = () => {
    if (!selectedId) return;
    setDeleteDialog(false);
    setDialogues((prev) => prev.filter((d) => d.id !== selectedId));
    setSelectedId(null);
  };

  const saveDialogues = async () => {
    setSaving(true);
    try {
      // Save as content collection "dialogues"
      try {
        await contentApi.createCollection('dialogues');
      } catch { /* may already exist */ }
      // Delete existing items, then save all
      const existing = await contentApi.listItems('dialogues').catch(() => []);
      for (const item of existing) {
        await contentApi.deleteItem('dialogues', item.id);
      }
      for (const d of dialogues) {
        await contentApi.updateItem('dialogues', d.id, d as unknown as Record<string, unknown> & { id: string });
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : '저장 실패');
    } finally {
      setSaving(false);
    }
  };

  const generateLua = async () => {
    try {
      await saveDialogues();
      const result = await dialogueApi.generate();
      setLuaPreview(result.preview);
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Lua 생성 실패');
    }
  };

  return (
    <div className="flex h-full">
      {error && (
        <div className="fixed top-4 right-4 bg-red-600 text-white px-4 py-2 rounded shadow-lg z-50">
          {error}
          <button className="ml-2 font-bold" onClick={() => setError(null)}>x</button>
        </div>
      )}

      <PromptDialog
        open={createDialog}
        title="새 대화"
        label="NPC 이름"
        placeholder="예: 경비병"
        onSubmit={handleCreate}
        onCancel={() => setCreateDialog(false)}
      />
      <ConfirmDialog
        open={deleteDialog}
        title="대화 삭제"
        message={`"${selected?.npc_name}" 대화를 삭제하시겠습니까?`}
        confirmLabel="삭제"
        onConfirm={handleDelete}
        onCancel={() => setDeleteDialog(false)}
      />

      {/* Left sidebar */}
      <div className="w-64 border-r border-gray-700 bg-gray-800 flex flex-col">
        <div className="p-3 border-b border-gray-700">
          <div className="flex items-center justify-between mb-2">
            <span className="text-sm font-medium text-gray-300">NPC 대화</span>
            <button
              onClick={() => setCreateDialog(true)}
              className="text-xs px-2 py-1 bg-blue-600 hover:bg-blue-500 rounded"
            >
              + 새로 만들기
            </button>
          </div>
          <div className="flex gap-1">
            <button
              onClick={saveDialogues}
              disabled={saving}
              className="flex-1 text-xs px-2 py-1 bg-green-700 hover:bg-green-600 disabled:opacity-50 rounded"
            >
              {saving ? '저장 중...' : '저장'}
            </button>
            <button
              onClick={generateLua}
              className="flex-1 text-xs px-2 py-1 bg-purple-700 hover:bg-purple-600 rounded"
            >
              Lua 생성
            </button>
          </div>
        </div>

        <div className="flex-1 overflow-y-auto">
          {dialogues.map((d) => (
            <button
              key={d.id}
              onClick={() => { setSelectedId(d.id); setLuaPreview(null); }}
              className={`w-full text-left px-3 py-2 text-sm border-b border-gray-700/50 transition-colors ${
                selectedId === d.id
                  ? 'bg-blue-900/40 text-blue-300'
                  : 'text-gray-400 hover:bg-gray-700/50'
              }`}
            >
              <div className="truncate">{d.npc_name}</div>
              <div className="text-[10px] text-gray-500">{d.nodes.length}개 노드</div>
            </button>
          ))}
          {dialogues.length === 0 && (
            <div className="p-3 text-xs text-gray-500 text-center">대화가 없습니다</div>
          )}
        </div>
      </div>

      {/* Right panel */}
      <div className="flex-1 flex flex-col overflow-hidden">
        {selected ? (
          <div className="flex-1 overflow-y-auto p-6">
            <DialogueForm
              dialogue={selected}
              onChange={updateDialogue}
              onDelete={() => setDeleteDialog(true)}
            />
          </div>
        ) : luaPreview ? (
          <div className="flex-1 overflow-y-auto">
            <div className="flex items-center justify-between px-4 py-2 bg-gray-800 border-b border-gray-700">
              <span className="text-sm text-gray-400">생성된 Lua 미리보기</span>
              <button onClick={() => setLuaPreview(null)} className="text-xs text-gray-500 hover:text-gray-300">닫기</button>
            </div>
            <pre className="p-4 text-xs font-mono text-green-300 whitespace-pre-wrap">{luaPreview}</pre>
          </div>
        ) : (
          <div className="flex items-center justify-center h-full text-gray-500 text-sm">
            편집할 대화를 선택하거나 새로 만드세요
          </div>
        )}
      </div>
    </div>
  );
}

// --- Dialogue Form ---

interface DialogueFormProps {
  dialogue: Dialogue;
  onChange: (d: Dialogue) => void;
  onDelete: () => void;
}

function DialogueForm({ dialogue, onChange, onDelete }: DialogueFormProps) {
  const update = (patch: Partial<Dialogue>) => {
    onChange({ ...dialogue, ...patch });
  };

  const updateNode = (index: number, node: DialogueNode) => {
    const nodes = [...dialogue.nodes];
    nodes[index] = node;
    update({ nodes });
  };

  const addNode = () => {
    const id = `node_${dialogue.nodes.length}`;
    update({ nodes: [...dialogue.nodes, { id, text: '', choices: [{ text: '', next: null }] }] });
  };

  const removeNode = (index: number) => {
    update({ nodes: dialogue.nodes.filter((_, i) => i !== index) });
  };

  const nodeIds = dialogue.nodes.map((n) => n.id);

  return (
    <div className="max-w-2xl space-y-6">
      <div className="flex items-center justify-between">
        <h2 className="text-xl font-bold">{dialogue.npc_name}</h2>
        <button onClick={onDelete} className="px-3 py-1 text-xs bg-red-700 hover:bg-red-600 rounded">삭제</button>
      </div>

      <div>
        <label className="block text-xs text-gray-400 mb-1">NPC 이름</label>
        <input
          type="text"
          value={dialogue.npc_name}
          onChange={(e) => update({ npc_name: e.target.value })}
          className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
        />
      </div>

      {/* Flow preview — SVG flowchart */}
      <DialogueFlowChart nodes={dialogue.nodes} />

      {/* Nodes */}
      <div className="space-y-4">
        <div className="flex items-center justify-between">
          <span className="text-xs font-bold text-yellow-400 bg-yellow-400/10 px-2 py-0.5 rounded">대화 노드</span>
          <button onClick={addNode} className="text-xs text-blue-400 hover:text-blue-300">+ 노드 추가</button>
        </div>

        {dialogue.nodes.map((node, ni) => (
          <div key={ni} className="bg-gray-800/50 border border-gray-700 rounded-lg p-4 space-y-3">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <span className="text-xs text-gray-500">노드</span>
                <input
                  type="text"
                  value={node.id}
                  onChange={(e) => updateNode(ni, { ...node, id: e.target.value })}
                  className="bg-gray-700 border border-gray-600 rounded px-2 py-0.5 text-xs w-32"
                />
              </div>
              {dialogue.nodes.length > 1 && (
                <button onClick={() => removeNode(ni)} className="text-gray-500 hover:text-red-400 text-xs">제거</button>
              )}
            </div>

            <div>
              <label className="block text-xs text-gray-400 mb-1">NPC 대사</label>
              <textarea
                value={node.text}
                onChange={(e) => updateNode(ni, { ...node, text: e.target.value })}
                rows={2}
                className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
                placeholder="NPC가 말할 내용..."
              />
            </div>

            {/* Choices */}
            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <span className="text-xs text-gray-500">선택지</span>
                <button
                  onClick={() => updateNode(ni, { ...node, choices: [...node.choices, { text: '', next: null }] })}
                  className="text-xs text-blue-400 hover:text-blue-300"
                >
                  + 선택지 추가
                </button>
              </div>
              {node.choices.map((choice, ci) => (
                <ChoiceEditor
                  key={ci}
                  choice={choice}
                  nodeIds={nodeIds}
                  onChange={(c) => {
                    const choices = [...node.choices];
                    choices[ci] = c;
                    updateNode(ni, { ...node, choices });
                  }}
                  onRemove={node.choices.length > 1 ? () => {
                    updateNode(ni, { ...node, choices: node.choices.filter((_, i) => i !== ci) });
                  } : undefined}
                />
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

// --- Choice Editor ---

function ChoiceEditor({
  choice,
  nodeIds,
  onChange,
  onRemove,
}: {
  choice: DialogueChoice;
  nodeIds: string[];
  onChange: (c: DialogueChoice) => void;
  onRemove?: () => void;
}) {
  return (
    <div className="flex items-start gap-2 bg-gray-700/30 border border-gray-700 rounded p-2">
      <div className="flex-1 space-y-1">
        <input
          type="text"
          value={choice.text}
          onChange={(e) => onChange({ ...choice, text: e.target.value })}
          placeholder="선택지 텍스트"
          className="w-full bg-gray-700 border border-gray-600 rounded px-2 py-1 text-xs"
        />
        <div className="flex gap-2">
          <select
            value={choice.next || ''}
            onChange={(e) => onChange({ ...choice, next: e.target.value || null })}
            className="flex-1 bg-gray-700 border border-gray-600 rounded px-2 py-1 text-xs"
          >
            <option value="">(대화 종료)</option>
            {nodeIds.map((nid) => (
              <option key={nid} value={nid}>{nid}</option>
            ))}
          </select>
          <input
            type="text"
            value={choice.action || ''}
            onChange={(e) => onChange({ ...choice, action: e.target.value || undefined })}
            placeholder="액션 (선택, 예: start_quest:goblin_hunt)"
            className="flex-1 bg-gray-700 border border-gray-600 rounded px-2 py-1 text-xs"
          />
        </div>
      </div>
      {onRemove && (
        <button onClick={onRemove} className="text-gray-500 hover:text-red-400 text-xs mt-1">x</button>
      )}
    </div>
  );
}

// --- Dialogue Flow Chart (SVG) ---

const NODE_W = 140;
const NODE_H = 50;
const GAP_X = 40;
const GAP_Y = 80;
const PADDING = 20;

function DialogueFlowChart({ nodes }: { nodes: DialogueNode[] }) {
  const layout = useMemo(() => {
    if (nodes.length === 0) return { positions: new Map<string, { x: number; y: number }>(), edges: [] as { from: string; to: string; label: string }[], width: 0, height: 0 };

    // BFS to assign positions
    const positions = new Map<string, { x: number; y: number }>();
    const idSet = new Set(nodes.map((n) => n.id));
    const visited = new Set<string>();
    const levels: string[][] = [];

    // Start from first node
    const queue: { id: string; level: number }[] = [{ id: nodes[0].id, level: 0 }];
    visited.add(nodes[0].id);

    while (queue.length > 0) {
      const { id, level } = queue.shift()!;
      if (!levels[level]) levels[level] = [];
      levels[level].push(id);

      const node = nodes.find((n) => n.id === id);
      if (node) {
        for (const choice of node.choices) {
          if (choice.next && idSet.has(choice.next) && !visited.has(choice.next)) {
            visited.add(choice.next);
            queue.push({ id: choice.next, level: level + 1 });
          }
        }
      }
    }

    // Add any unvisited nodes
    for (const node of nodes) {
      if (!visited.has(node.id)) {
        if (!levels[levels.length]) levels.push([]);
        levels[levels.length - 1].push(node.id);
      }
    }

    // Calculate positions
    let maxWidth = 0;
    for (let lvl = 0; lvl < levels.length; lvl++) {
      const row = levels[lvl];
      const totalW = row.length * NODE_W + (row.length - 1) * GAP_X;
      if (totalW > maxWidth) maxWidth = totalW;
      for (let i = 0; i < row.length; i++) {
        positions.set(row[i], {
          x: PADDING + i * (NODE_W + GAP_X),
          y: PADDING + lvl * (NODE_H + GAP_Y),
        });
      }
    }

    // Build edges
    const edges: { from: string; to: string; label: string }[] = [];
    for (const node of nodes) {
      for (const choice of node.choices) {
        if (choice.next && idSet.has(choice.next)) {
          edges.push({ from: node.id, to: choice.next, label: choice.text.length > 12 ? choice.text.slice(0, 12) + '...' : choice.text });
        }
      }
    }

    const width = maxWidth + PADDING * 2;
    const height = levels.length * (NODE_H + GAP_Y) - GAP_Y + PADDING * 2;

    return { positions, edges, width, height };
  }, [nodes]);

  if (nodes.length === 0) return null;

  return (
    <div className="bg-gray-800/50 border border-gray-700 rounded-lg p-3">
      <div className="text-xs text-gray-400 mb-2">대화 플로우</div>
      <div className="overflow-auto max-h-64">
        <svg width={Math.max(layout.width, 200)} height={Math.max(layout.height, 80)} className="text-xs">
          <defs>
            <marker id="flowArrow" viewBox="0 0 10 7" refX="10" refY="3.5" markerWidth="8" markerHeight="6" orient="auto-start-reverse">
              <polygon points="0 0, 10 3.5, 0 7" fill="#6b7280" />
            </marker>
          </defs>

          {/* Edges */}
          {layout.edges.map((edge, i) => {
            const from = layout.positions.get(edge.from);
            const to = layout.positions.get(edge.to);
            if (!from || !to) return null;
            const x1 = from.x + NODE_W / 2;
            const y1 = from.y + NODE_H;
            const x2 = to.x + NODE_W / 2;
            const y2 = to.y;
            const midY = (y1 + y2) / 2;
            return (
              <g key={i}>
                <path
                  d={`M${x1},${y1} C${x1},${midY} ${x2},${midY} ${x2},${y2}`}
                  fill="none"
                  stroke="#6b7280"
                  strokeWidth={1.5}
                  markerEnd="url(#flowArrow)"
                />
                {edge.label && (
                  <text x={(x1 + x2) / 2} y={midY - 4} textAnchor="middle" fill="#9ca3af" fontSize={9}>
                    {edge.label}
                  </text>
                )}
              </g>
            );
          })}

          {/* Nodes */}
          {nodes.map((node) => {
            const pos = layout.positions.get(node.id);
            if (!pos) return null;
            const isStart = node.id === nodes[0]?.id;
            const isEnd = node.choices.every((c) => !c.next);
            const borderColor = isStart ? '#22c55e' : isEnd ? '#ef4444' : '#3b82f6';
            return (
              <g key={node.id}>
                <rect
                  x={pos.x}
                  y={pos.y}
                  width={NODE_W}
                  height={NODE_H}
                  rx={6}
                  fill="#1f2937"
                  stroke={borderColor}
                  strokeWidth={1.5}
                />
                <text x={pos.x + NODE_W / 2} y={pos.y + 18} textAnchor="middle" fill={borderColor} fontSize={11} fontWeight="bold">
                  {node.id}
                </text>
                <text x={pos.x + NODE_W / 2} y={pos.y + 34} textAnchor="middle" fill="#9ca3af" fontSize={9}>
                  {node.text.length > 16 ? node.text.slice(0, 16) + '...' : node.text || '(빈 대사)'}
                </text>
              </g>
            );
          })}
        </svg>
      </div>
      <div className="flex gap-3 mt-2 text-[10px] text-gray-500">
        <span><span className="inline-block w-2 h-2 rounded-full bg-green-500 mr-1" />시작</span>
        <span><span className="inline-block w-2 h-2 rounded-full bg-blue-500 mr-1" />중간</span>
        <span><span className="inline-block w-2 h-2 rounded-full bg-red-500 mr-1" />종료</span>
      </div>
    </div>
  );
}
