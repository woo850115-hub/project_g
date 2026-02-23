import { useCallback, useEffect, useState } from 'react';
import { contentApi, questApi } from '../api/client';
import type { Quest, QuestObjective, QuestRewards } from '../types/quest';
import type { ContentItem } from '../types/content';
import { PromptDialog, ConfirmDialog } from '../components/Modal';

export function QuestEditor() {
  const [quests, setQuests] = useState<Quest[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [luaPreview, setLuaPreview] = useState<string | null>(null);
  const [contentItems, setContentItems] = useState<Record<string, ContentItem[]>>({});

  const [createDialog, setCreateDialog] = useState(false);
  const [deleteDialog, setDeleteDialog] = useState(false);

  const loadQuests = useCallback(async () => {
    try {
      const data = await contentApi.listItems('quests');
      setQuests(data as unknown as Quest[]);
    } catch {
      setQuests([]);
    }
  }, []);

  const loadContent = useCallback(async () => {
    try {
      const cols = await contentApi.listCollections();
      const items: Record<string, ContentItem[]> = {};
      for (const col of cols) {
        try { items[col] = await contentApi.listItems(col); } catch { /* skip */ }
      }
      setContentItems(items);
    } catch { /* ignore */ }
  }, []);

  useEffect(() => {
    loadQuests();
    loadContent();
  }, [loadQuests, loadContent]);

  const selected = quests.find((q) => q.id === selectedId) || null;

  const updateQuest = (updated: Quest) => {
    setQuests((prev) => prev.map((q) => (q.id === updated.id ? updated : q)));
  };

  const handleCreate = (name: string) => {
    setCreateDialog(false);
    const id = name.toLowerCase().replace(/[^a-z0-9]+/g, '_').replace(/^_|_$/g, '');
    if (quests.some((q) => q.id === id)) {
      setError(`퀘스트 ID "${id}"이(가) 이미 존재합니다`);
      return;
    }
    const newQuest: Quest = {
      id,
      name,
      description: '',
      npc_name: '',
      auto_complete: true,
      objectives: [{ type: 'kill', target: '', count: 1 }],
      rewards: { gold: 0, exp: 0, items: [] },
    };
    setQuests((prev) => [...prev, newQuest]);
    setSelectedId(id);
  };

  const handleDelete = () => {
    if (!selectedId) return;
    setDeleteDialog(false);
    setQuests((prev) => prev.filter((q) => q.id !== selectedId));
    setSelectedId(null);
  };

  const saveQuests = async () => {
    setSaving(true);
    try {
      try { await contentApi.createCollection('quests'); } catch { /* may exist */ }
      const existing = await contentApi.listItems('quests').catch(() => []);
      for (const item of existing) {
        await contentApi.deleteItem('quests', item.id);
      }
      for (const q of quests) {
        await contentApi.updateItem('quests', q.id, q as unknown as Record<string, unknown> & { id: string });
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : '저장 실패');
    } finally {
      setSaving(false);
    }
  };

  const generateLua = async () => {
    try {
      await saveQuests();
      const result = await questApi.generate();
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
        title="새 퀘스트"
        label="퀘스트 이름"
        placeholder="예: 고블린 퇴치"
        onSubmit={handleCreate}
        onCancel={() => setCreateDialog(false)}
      />
      <ConfirmDialog
        open={deleteDialog}
        title="퀘스트 삭제"
        message={`"${selected?.name}" 퀘스트를 삭제하시겠습니까?`}
        confirmLabel="삭제"
        onConfirm={handleDelete}
        onCancel={() => setDeleteDialog(false)}
      />

      {/* Left sidebar */}
      <div className="w-64 border-r border-gray-700 bg-gray-800 flex flex-col">
        <div className="p-3 border-b border-gray-700">
          <div className="flex items-center justify-between mb-2">
            <span className="text-sm font-medium text-gray-300">퀘스트</span>
            <button
              onClick={() => setCreateDialog(true)}
              className="text-xs px-2 py-1 bg-blue-600 hover:bg-blue-500 rounded"
            >
              + 새로 만들기
            </button>
          </div>
          <div className="flex gap-1">
            <button
              onClick={saveQuests}
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
          {quests.map((q) => (
            <button
              key={q.id}
              onClick={() => { setSelectedId(q.id); setLuaPreview(null); }}
              className={`w-full text-left px-3 py-2 text-sm border-b border-gray-700/50 transition-colors ${
                selectedId === q.id
                  ? 'bg-blue-900/40 text-blue-300'
                  : 'text-gray-400 hover:bg-gray-700/50'
              }`}
            >
              <div className="truncate">{q.name}</div>
              <div className="text-[10px] text-gray-500">{q.objectives.length}개 목표</div>
            </button>
          ))}
          {quests.length === 0 && (
            <div className="p-3 text-xs text-gray-500 text-center">퀘스트가 없습니다</div>
          )}
        </div>
      </div>

      {/* Right panel */}
      <div className="flex-1 flex flex-col overflow-hidden">
        {selected ? (
          <div className="flex-1 overflow-y-auto p-6">
            <QuestForm
              quest={selected}
              contentItems={contentItems}
              onChange={updateQuest}
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
            편집할 퀘스트를 선택하거나 새로 만드세요
          </div>
        )}
      </div>
    </div>
  );
}

// --- Quest Form ---

interface QuestFormProps {
  quest: Quest;
  contentItems: Record<string, ContentItem[]>;
  onChange: (q: Quest) => void;
  onDelete: () => void;
}

const OBJECTIVE_TYPES = [
  { value: 'kill', label: '몬스터 처치' },
  { value: 'collect', label: '아이템 수집' },
  { value: 'visit', label: '방 방문' },
  { value: 'talk', label: 'NPC 대화' },
] as const;

function QuestForm({ quest, contentItems, onChange, onDelete }: QuestFormProps) {
  const update = (patch: Partial<Quest>) => {
    onChange({ ...quest, ...patch });
  };

  const updateObjective = (index: number, obj: QuestObjective) => {
    const objectives = [...quest.objectives];
    objectives[index] = obj;
    update({ objectives });
  };

  const addObjective = () => {
    update({ objectives: [...quest.objectives, { type: 'kill', target: '', count: 1 }] });
  };

  const removeObjective = (index: number) => {
    update({ objectives: quest.objectives.filter((_, i) => i !== index) });
  };

  const updateRewards = (patch: Partial<QuestRewards>) => {
    update({ rewards: { ...quest.rewards, ...patch } });
  };

  const monsters = contentItems['monsters'] || [];
  const items = contentItems['items'] || [];

  return (
    <div className="max-w-2xl space-y-6">
      <div className="flex items-center justify-between">
        <h2 className="text-xl font-bold">{quest.name}</h2>
        <button onClick={onDelete} className="px-3 py-1 text-xs bg-red-700 hover:bg-red-600 rounded">삭제</button>
      </div>

      <div className="grid grid-cols-2 gap-4">
        <div>
          <label className="block text-xs text-gray-400 mb-1">퀘스트 이름</label>
          <input
            type="text"
            value={quest.name}
            onChange={(e) => update({ name: e.target.value })}
            className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
          />
        </div>
        <div>
          <label className="block text-xs text-gray-400 mb-1">NPC 이름</label>
          <input
            type="text"
            value={quest.npc_name}
            onChange={(e) => update({ npc_name: e.target.value })}
            placeholder="퀘스트를 주는 NPC"
            className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
          />
        </div>
      </div>

      <div>
        <label className="block text-xs text-gray-400 mb-1">설명</label>
        <textarea
          value={quest.description}
          onChange={(e) => update({ description: e.target.value })}
          rows={2}
          className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
          placeholder="퀘스트 설명 텍스트"
        />
      </div>

      <label className="flex items-center gap-2 text-sm text-gray-300">
        <input
          type="checkbox"
          checked={quest.auto_complete}
          onChange={(e) => update({ auto_complete: e.target.checked })}
          className="rounded"
        />
        자동 완료 (목표 달성 시 즉시 보상)
      </label>

      {/* Objectives */}
      <div className="space-y-3">
        <div className="flex items-center justify-between">
          <span className="text-xs font-bold text-yellow-400 bg-yellow-400/10 px-2 py-0.5 rounded">목표</span>
          <button onClick={addObjective} className="text-xs text-blue-400 hover:text-blue-300">+ 목표 추가</button>
        </div>

        {quest.objectives.map((obj, i) => (
          <div key={i} className="bg-gray-800/50 border border-gray-700 rounded-lg p-3 space-y-2">
            <div className="flex items-center justify-between">
              <select
                value={obj.type}
                onChange={(e) => updateObjective(i, { ...obj, type: e.target.value as QuestObjective['type'], target: '' })}
                className="bg-gray-700 border border-gray-600 rounded px-2 py-1 text-sm"
              >
                {OBJECTIVE_TYPES.map((ot) => (
                  <option key={ot.value} value={ot.value}>{ot.label}</option>
                ))}
              </select>
              {quest.objectives.length > 1 && (
                <button onClick={() => removeObjective(i)} className="text-gray-500 hover:text-red-400 text-xs">제거</button>
              )}
            </div>

            <div className="grid grid-cols-2 gap-2">
              <div>
                <label className="block text-xs text-gray-400 mb-1">대상</label>
                {obj.type === 'kill' && monsters.length > 0 ? (
                  <select
                    value={obj.target}
                    onChange={(e) => updateObjective(i, { ...obj, target: e.target.value })}
                    className="w-full bg-gray-700 border border-gray-600 rounded px-2 py-1 text-xs"
                  >
                    <option value="">-- 선택 --</option>
                    {monsters.map((m) => (
                      <option key={m.id} value={String(m.name || m.id)}>{String(m.name || m.id)}</option>
                    ))}
                  </select>
                ) : obj.type === 'collect' && items.length > 0 ? (
                  <select
                    value={obj.target}
                    onChange={(e) => updateObjective(i, { ...obj, target: e.target.value })}
                    className="w-full bg-gray-700 border border-gray-600 rounded px-2 py-1 text-xs"
                  >
                    <option value="">-- 선택 --</option>
                    {items.map((item) => (
                      <option key={item.id} value={String(item.name || item.id)}>{String(item.name || item.id)}</option>
                    ))}
                  </select>
                ) : (
                  <input
                    type="text"
                    value={obj.target}
                    onChange={(e) => updateObjective(i, { ...obj, target: e.target.value })}
                    placeholder={obj.type === 'visit' ? '방 이름' : obj.type === 'talk' ? 'NPC 이름' : '대상 이름'}
                    className="w-full bg-gray-700 border border-gray-600 rounded px-2 py-1 text-xs"
                  />
                )}
              </div>
              {obj.type !== 'talk' && (
                <div>
                  <label className="block text-xs text-gray-400 mb-1">수량</label>
                  <input
                    type="number"
                    value={obj.count}
                    onChange={(e) => updateObjective(i, { ...obj, count: Math.max(1, Number(e.target.value) || 1) })}
                    min={1}
                    className="w-full bg-gray-700 border border-gray-600 rounded px-2 py-1 text-xs"
                  />
                </div>
              )}
            </div>
          </div>
        ))}
      </div>

      {/* Rewards */}
      <div className="bg-gray-800/50 border border-gray-700 rounded-lg p-4 space-y-3">
        <span className="text-xs font-bold text-green-400 bg-green-400/10 px-2 py-0.5 rounded">보상</span>

        <div className="grid grid-cols-2 gap-4">
          <div>
            <label className="block text-xs text-gray-400 mb-1">골드</label>
            <input
              type="number"
              value={quest.rewards.gold}
              onChange={(e) => updateRewards({ gold: Number(e.target.value) || 0 })}
              min={0}
              className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
            />
          </div>
          <div>
            <label className="block text-xs text-gray-400 mb-1">경험치</label>
            <input
              type="number"
              value={quest.rewards.exp}
              onChange={(e) => updateRewards({ exp: Number(e.target.value) || 0 })}
              min={0}
              className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
            />
          </div>
        </div>

        <div>
          <label className="block text-xs text-gray-400 mb-1">보상 아이템</label>
          <div className="flex flex-wrap gap-1.5 mb-2 min-h-[28px]">
            {quest.rewards.items.map((itemName, i) => (
              <span key={i} className="inline-flex items-center gap-1 bg-gray-700 border border-gray-600 rounded px-2 py-0.5 text-xs">
                {itemName}
                <button
                  onClick={() => updateRewards({ items: quest.rewards.items.filter((_, j) => j !== i) })}
                  className="text-gray-500 hover:text-red-400"
                >&times;</button>
              </span>
            ))}
          </div>
          {items.length > 0 ? (
            <select
              value=""
              onChange={(e) => {
                if (e.target.value) {
                  updateRewards({ items: [...quest.rewards.items, e.target.value] });
                  e.target.value = '';
                }
              }}
              className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm text-gray-400"
            >
              <option value="">+ 아이템 추가...</option>
              {items.map((item) => (
                <option key={item.id} value={String(item.name || item.id)}>{String(item.name || item.id)}</option>
              ))}
            </select>
          ) : (
            <input
              type="text"
              placeholder="아이템 이름 입력 후 Enter"
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  const v = (e.target as HTMLInputElement).value.trim();
                  if (v) {
                    updateRewards({ items: [...quest.rewards.items, v] });
                    (e.target as HTMLInputElement).value = '';
                  }
                }
              }}
              className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
            />
          )}
        </div>
      </div>
    </div>
  );
}
