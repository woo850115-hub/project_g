import { useCallback, useEffect, useState } from 'react';
import { triggerApi, contentApi, worldApi } from '../api/client';
import type { Trigger, TriggerCondition, TriggerAction } from '../types/trigger';
import type { Room } from '../types/world';
import type { ContentItem } from '../types/content';
import { PromptDialog, ConfirmDialog } from '../components/Modal';
import { Tooltip } from '../components/Tooltip';

const CONDITION_TYPES = [
  { value: 'enter_room', label: '\uBC29 \uC785\uC7A5', desc: '\uD50C\uB808\uC774\uC5B4\uAC00 \uD2B9\uC815 \uBC29\uC5D0 \uB4E4\uC5B4\uC62C \uB54C \uBC1C\uB3D9' },
  { value: 'command', label: '\uD50C\uB808\uC774\uC5B4 \uBA85\uB839', desc: '\uD50C\uB808\uC774\uC5B4\uAC00 \uD2B9\uC815 \uBA85\uB839\uC5B4\uB97C \uC785\uB825\uD560 \uB54C \uBC1C\uB3D9' },
  { value: 'tick_interval', label: '\uD0C0\uC774\uBA38', desc: '\uC77C\uC815 \uD2F1 \uAC04\uACA9\uC73C\uB85C \uBC18\uBCF5 \uBC1C\uB3D9 (20\uD2F1 = 1\uCD08)' },
  { value: 'entity_death', label: '\uC5D4\uD2F0\uD2F0 \uC0AC\uB9DD', desc: '\uC5D4\uD2F0\uD2F0\uAC00 \uC8FD\uC5C8\uC744 \uB54C \uBC1C\uB3D9' },
  { value: 'on_connect', label: '\uD50C\uB808\uC774\uC5B4 \uC811\uC18D', desc: '\uD50C\uB808\uC774\uC5B4\uAC00 \uC11C\uBC84\uC5D0 \uC811\uC18D\uD560 \uB54C \uBC1C\uB3D9' },
] as const;

// ECS component list for set_component action
// inputType: 'number' | 'string' | 'boolean' | 'health' | 'level' | 'skills'
const ECS_COMPONENTS: {
  value: string; label: string; inputType: string;
}[] = [
  { value: 'Name', label: '이름 (Name)', inputType: 'string' },
  { value: 'Description', label: '설명 (Description)', inputType: 'string' },
  { value: 'Health', label: '체력 (Health)', inputType: 'health' },
  { value: 'Attack', label: '공격력 (Attack)', inputType: 'number' },
  { value: 'Defense', label: '방어력 (Defense)', inputType: 'number' },
  { value: 'Gold', label: '골드 (Gold)', inputType: 'number' },
  { value: 'Race', label: '종족 (Race)', inputType: 'string' },
  { value: 'Class', label: '직업 (Class)', inputType: 'string' },
  { value: 'Level', label: '레벨 (Level)', inputType: 'level' },
  { value: 'Skills', label: '스킬 (Skills)', inputType: 'skills' },
  { value: 'Dead', label: '사망 (Dead)', inputType: 'boolean' },
  { value: 'PlayerTag', label: '플레이어 태그 (PlayerTag)', inputType: 'boolean' },
  { value: 'NpcTag', label: 'NPC 태그 (NpcTag)', inputType: 'boolean' },
  { value: 'ItemTag', label: '아이템 태그 (ItemTag)', inputType: 'boolean' },
];

// Default value for each component when selected
function defaultComponentValue(comp: string): unknown {
  switch (comp) {
    case 'Health': return { current: 100, max: 100 };
    case 'Level': return { level: 1, exp: 0, exp_next: 100 };
    case 'Skills': return { learned: [] };
    case 'Attack': case 'Defense': case 'Gold': return 0;
    case 'Dead': case 'PlayerTag': case 'NpcTag': case 'ItemTag': return true;
    default: return '';
  }
}

const ACTION_TYPES = [
  { value: 'send_message', label: '\uBA54\uC2DC\uC9C0 \uC804\uC1A1', desc: '\uD50C\uB808\uC774\uC5B4\uB098 \uBC29 \uC804\uCCB4\uC5D0 \uD14D\uC2A4\uD2B8 \uBA54\uC2DC\uC9C0\uB97C \uBCF4\uB0C5\uB2C8\uB2E4' },
  { value: 'spawn_entity', label: '\uC5D4\uD2F0\uD2F0 \uC0DD\uC131', desc: '\uC9C0\uC815\uD55C \uBC29\uC5D0 NPC\uB098 \uC544\uC774\uD15C\uC744 \uC0DD\uC131\uD569\uB2C8\uB2E4' },
  { value: 'teleport', label: '\uD50C\uB808\uC774\uC5B4 \uC774\uB3D9', desc: '\uD50C\uB808\uC774\uC5B4\uB97C \uC9C0\uC815\uD55C \uBC29\uC73C\uB85C \uC774\uB3D9\uC2DC\uD0B5\uB2C8\uB2E4' },
  { value: 'give_item', label: '\uC544\uC774\uD15C \uC9C0\uAE09', desc: '\uD50C\uB808\uC774\uC5B4\uC5D0\uAC8C \uC544\uC774\uD15C\uC744 \uC9C0\uAE09\uD569\uB2C8\uB2E4' },
  { value: 'heal', label: '체력 회복', desc: '엔티티의 체력을 회복시킵니다 (최대 체력 초과 불가)' },
  { value: 'set_component', label: '컴포넌트 설정', desc: '엔티티의 ECS 컴포넌트 값을 설정합니다' },
  { value: 'despawn_trigger_entity', label: '엔티티 제거', desc: '트리거를 발동시킨 엔티티를 제거합니다' },
] as const;

function makeDefaultCondition(type: string): TriggerCondition {
  switch (type) {
    case 'enter_room': return { type: 'enter_room', room_id: '' };
    case 'command': return { type: 'command', command: '' };
    case 'tick_interval': return { type: 'tick_interval', interval: 60 };
    case 'entity_death': return { type: 'entity_death', content_id: '' };
    case 'on_connect': return { type: 'on_connect' };
    default: return { type: 'on_connect' };
  }
}

function makeDefaultAction(type: string): TriggerAction {
  switch (type) {
    case 'send_message': return { type: 'send_message', target: 'player', text: '' };
    case 'spawn_entity': return { type: 'spawn_entity', entity_type: 'npc', content_id: '', room_id: '' };
    case 'teleport': return { type: 'teleport', room_id: '' };
    case 'give_item': return { type: 'give_item', content_id: '' };
    case 'heal': return { type: 'heal', target: 'player', mode: 'full', amount: 0 };
    case 'set_component': return { type: 'set_component', target: 'player', component: '', value: '' };
    case 'despawn_trigger_entity': return { type: 'despawn_trigger_entity' };
    default: return { type: 'send_message', target: 'player', text: '' };
  }
}

export function TriggerEditor() {
  const [triggers, setTriggers] = useState<Trigger[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [luaPreview, setLuaPreview] = useState<string | null>(null);
  const [rooms, setRooms] = useState<Room[]>([]);
  const [contentItems, setContentItems] = useState<Record<string, ContentItem[]>>({});

  // Dialogs
  const [createDialog, setCreateDialog] = useState(false);
  const [deleteDialog, setDeleteDialog] = useState(false);

  const loadTriggers = useCallback(async () => {
    try {
      const data = await triggerApi.list();
      setTriggers(data);
    } catch (e) {
      setError(e instanceof Error ? e.message : '\uD2B8\uB9AC\uAC70 \uBD88\uB7EC\uC624\uAE30 \uC2E4\uD328');
    }
  }, []);

  const loadContent = useCallback(async () => {
    try {
      const world = await worldApi.get();
      setRooms(world.rooms);
    } catch { /* ignore */ }
    try {
      const cols = await contentApi.listCollections();
      const items: Record<string, ContentItem[]> = {};
      for (const col of cols) {
        try {
          items[col] = await contentApi.listItems(col);
        } catch { /* skip */ }
      }
      setContentItems(items);
    } catch { /* ignore */ }
  }, []);

  useEffect(() => {
    loadTriggers();
    loadContent();
  }, [loadTriggers, loadContent]);

  const selected = triggers.find((t) => t.id === selectedId) || null;

  const updateTrigger = (updated: Trigger) => {
    setTriggers((prev) =>
      prev.map((t) => (t.id === updated.id ? updated : t))
    );
  };

  const handleCreate = (name: string) => {
    setCreateDialog(false);
    const id = name
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '_')
      .replace(/^_|_$/g, '');
    if (triggers.some((t) => t.id === id)) {
      setError(`트리거 ID "${id}"이(가) 이미 존재합니다`);
      return;
    }
    const newTrigger: Trigger = {
      id,
      name,
      enabled: true,
      condition: { type: 'enter_room', room_id: '' },
      actions: [{ type: 'send_message', target: 'player', text: '' }],
    };
    setTriggers((prev) => [...prev, newTrigger]);
    setSelectedId(id);
  };

  const handleDelete = () => {
    if (!selectedId) return;
    setDeleteDialog(false);
    setTriggers((prev) => prev.filter((t) => t.id !== selectedId));
    setSelectedId(null);
  };

  const saveTriggers = async () => {
    setSaving(true);
    try {
      await triggerApi.save(triggers);
    } catch (e) {
      setError(e instanceof Error ? e.message : '\uC800\uC7A5 \uC2E4\uD328');
    } finally {
      setSaving(false);
    }
  };

  const generateLua = async () => {
    try {
      await triggerApi.save(triggers);
      const result = await triggerApi.generate();
      setLuaPreview(result.preview);
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Lua \uC0DD\uC131 \uC2E4\uD328');
    }
  };

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
      <PromptDialog
        open={createDialog}
        title="새 트리거"
        label="트리거 이름"
        placeholder="예: 던전 경고"
        onSubmit={handleCreate}
        onCancel={() => setCreateDialog(false)}
      />
      <ConfirmDialog
        open={deleteDialog}
        title="트리거 삭제"
        message={`"${selected?.name}" 트리거를 삭제하시겠습니까?`}
        confirmLabel="삭제"
        onConfirm={handleDelete}
        onCancel={() => setDeleteDialog(false)}
      />

      {/* Left sidebar — trigger list */}
      <div className="w-64 border-r border-gray-700 bg-gray-800 flex flex-col">
        <div className="p-3 border-b border-gray-700">
          <div className="flex items-center justify-between mb-2">
            <span className="text-sm font-medium text-gray-300">트리거</span>
            <button
              onClick={() => setCreateDialog(true)}
              className="text-xs px-2 py-1 bg-blue-600 hover:bg-blue-500 rounded"
            >
              + 새로 만들기
            </button>
          </div>
          <div className="flex gap-1">
            <button
              onClick={saveTriggers}
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
          {triggers.map((t) => (
            <button
              key={t.id}
              onClick={() => setSelectedId(t.id)}
              className={`w-full text-left px-3 py-2 text-sm border-b border-gray-700/50 transition-colors ${
                selectedId === t.id
                  ? 'bg-blue-900/40 text-blue-300'
                  : 'text-gray-400 hover:bg-gray-700/50'
              }`}
            >
              <div className="flex items-center gap-2">
                <span className={`w-2 h-2 rounded-full ${t.enabled ? 'bg-green-400' : 'bg-gray-600'}`} />
                <span className="truncate">{t.name}</span>
              </div>
              <div className="text-[10px] text-gray-500 ml-4">
                {CONDITION_TYPES.find((c) => c.value === t.condition.type)?.label}
              </div>
            </button>
          ))}
          {triggers.length === 0 && (
            <div className="p-3 text-xs text-gray-500 text-center">
              트리거가 없습니다
            </div>
          )}
        </div>
      </div>

      {/* Right panel — trigger editor */}
      <div className="flex-1 flex flex-col overflow-hidden">
        {selected ? (
          <div className="flex-1 overflow-y-auto p-6">
            <TriggerForm
              trigger={selected}
              rooms={rooms}
              contentItems={contentItems}
              onChange={updateTrigger}
              onDelete={() => setDeleteDialog(true)}
            />
          </div>
        ) : luaPreview ? (
          <div className="flex-1 overflow-y-auto">
            <div className="flex items-center justify-between px-4 py-2 bg-gray-800 border-b border-gray-700">
              <span className="text-sm text-gray-400">생성된 Lua 미리보기</span>
              <button
                onClick={() => setLuaPreview(null)}
                className="text-xs text-gray-500 hover:text-gray-300"
              >
                닫기
              </button>
            </div>
            <pre className="p-4 text-xs font-mono text-green-300 whitespace-pre-wrap">
              {luaPreview}
            </pre>
          </div>
        ) : (
          <div className="flex items-center justify-center h-full text-gray-500 text-sm">
            편집할 트리거를 선택하거나 새로 만드세요
          </div>
        )}
      </div>
    </div>
  );
}

// --- Trigger Form ---

interface TriggerFormProps {
  trigger: Trigger;
  rooms: Room[];
  contentItems: Record<string, ContentItem[]>;
  onChange: (trigger: Trigger) => void;
  onDelete: () => void;
}

function TriggerForm({ trigger, rooms, contentItems, onChange, onDelete }: TriggerFormProps) {
  const update = (patch: Partial<Trigger>) => {
    onChange({ ...trigger, ...patch });
  };

  const updateCondition = (condition: TriggerCondition) => {
    update({ condition });
  };

  const updateAction = (index: number, action: TriggerAction) => {
    const actions = [...trigger.actions];
    actions[index] = action;
    update({ actions });
  };

  const addAction = () => {
    update({ actions: [...trigger.actions, makeDefaultAction('send_message')] });
  };

  const removeAction = (index: number) => {
    update({ actions: trigger.actions.filter((_, i) => i !== index) });
  };

  return (
    <div className="max-w-2xl space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <h2 className="text-xl font-bold">{trigger.name}</h2>
          <label className="flex items-center gap-1.5 text-xs text-gray-400">
            <input
              type="checkbox"
              checked={trigger.enabled}
              onChange={(e) => update({ enabled: e.target.checked })}
              className="rounded"
            />
            활성화
          </label>
        </div>
        <button
          onClick={onDelete}
          className="px-3 py-1 text-xs bg-red-700 hover:bg-red-600 rounded"
        >
          삭제
        </button>
      </div>

      {/* Name */}
      <div>
        <label className="block text-xs text-gray-400 mb-1">이름</label>
        <input
          type="text"
          value={trigger.name}
          onChange={(e) => update({ name: e.target.value })}
          className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
        />
      </div>

      {/* WHEN — Condition */}
      <div className="bg-gray-800/50 border border-gray-700 rounded-lg p-4">
        <div className="flex items-center gap-2 mb-3">
          <Tooltip text="트리거가 발동되는 조건을 설정합니다">
            <span className="text-xs font-bold text-yellow-400 bg-yellow-400/10 px-2 py-0.5 rounded">조건</span>
          </Tooltip>
          <Tooltip text={CONDITION_TYPES.find((c) => c.value === trigger.condition.type)?.desc || ''}>
            <select
              value={trigger.condition.type}
              onChange={(e) => updateCondition(makeDefaultCondition(e.target.value))}
              className="bg-gray-700 border border-gray-600 rounded px-2 py-1 text-sm"
            >
              {CONDITION_TYPES.map((c) => (
                <option key={c.value} value={c.value}>{c.label}</option>
              ))}
            </select>
          </Tooltip>
        </div>

        <ConditionFields
          condition={trigger.condition}
          rooms={rooms}
          contentItems={contentItems}
          onChange={updateCondition}
        />
      </div>

      {/* THEN — Actions */}
      <div className="space-y-3">
        <div className="flex items-center justify-between">
          <Tooltip text="조건이 충족되면 순서대로 실행되는 동작입니다">
            <span className="text-xs font-bold text-green-400 bg-green-400/10 px-2 py-0.5 rounded">실행</span>
          </Tooltip>
          <button
            onClick={addAction}
            className="text-xs text-blue-400 hover:text-blue-300"
          >
            + 액션 추가
          </button>
        </div>

        {trigger.actions.map((action, i) => (
          <div key={i} className="bg-gray-800/50 border border-gray-700 rounded-lg p-4">
            <div className="flex items-center justify-between mb-3">
              <div className="flex items-center gap-2">
                <span className="text-xs text-gray-500">액션 {i + 1}</span>
                <Tooltip text={ACTION_TYPES.find((a) => a.value === action.type)?.desc || ''}>
                  <select
                    value={action.type}
                    onChange={(e) => updateAction(i, makeDefaultAction(e.target.value))}
                    className="bg-gray-700 border border-gray-600 rounded px-2 py-1 text-sm"
                  >
                    {ACTION_TYPES.map((a) => (
                      <option key={a.value} value={a.value}>{a.label}</option>
                    ))}
                  </select>
                </Tooltip>
              </div>
              {trigger.actions.length > 1 && (
                <button
                  onClick={() => removeAction(i)}
                  className="text-gray-500 hover:text-red-400 text-xs"
                >
                  제거
                </button>
              )}
            </div>

            <ActionFields
              action={action}
              rooms={rooms}
              contentItems={contentItems}
              onChange={(a) => updateAction(i, a)}
            />
          </div>
        ))}
      </div>
    </div>
  );
}

// --- Condition Fields ---

interface ConditionFieldsProps {
  condition: TriggerCondition;
  rooms: Room[];
  contentItems: Record<string, ContentItem[]>;
  onChange: (condition: TriggerCondition) => void;
}

function ConditionFields({ condition, rooms, contentItems, onChange }: ConditionFieldsProps) {
  switch (condition.type) {
    case 'enter_room':
      return (
        <div>
          <label className="block text-xs text-gray-400 mb-1">방</label>
          <select
            value={condition.room_id}
            onChange={(e) => onChange({ ...condition, room_id: e.target.value })}
            className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
          >
            <option value="">-- 방 선택 --</option>
            {rooms.map((r) => (
              <option key={r.id} value={r.name || r.id}>{r.name || r.id}</option>
            ))}
          </select>
        </div>
      );

    case 'command':
      return (
        <div>
          <label className="block text-xs text-gray-400 mb-1">명령어</label>
          <input
            type="text"
            value={condition.command}
            onChange={(e) => onChange({ ...condition, command: e.target.value })}
            placeholder="예: pray, search, talk"
            className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
          />
        </div>
      );

    case 'tick_interval':
      return (
        <div>
          <label className="block text-xs text-gray-400 mb-1">
            간격 (틱) — 20틱 = 1초
          </label>
          <input
            type="number"
            value={condition.interval}
            onChange={(e) => onChange({ ...condition, interval: Number(e.target.value) || 1 })}
            min={1}
            className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
          />
        </div>
      );

    case 'entity_death':
      return (
        <div>
          <label className="block text-xs text-gray-400 mb-1">엔티티 (콘텐츠 ID, 비워두면 모두 해당)</label>
          <select
            value={condition.content_id}
            onChange={(e) => onChange({ ...condition, content_id: e.target.value })}
            className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
          >
            <option value="">모든 엔티티</option>
            {(contentItems['monsters'] || []).map((m) => (
              <option key={m.id} value={String(m.name || m.id)}>
                {String(m.name || m.id)}
              </option>
            ))}
          </select>
        </div>
      );

    case 'on_connect':
      return (
        <p className="text-xs text-gray-500">
          플레이어가 서버에 접속할 때 발동됩니다.
        </p>
      );
  }
}

// --- Action Fields ---

interface ActionFieldsProps {
  action: TriggerAction;
  rooms: Room[];
  contentItems: Record<string, ContentItem[]>;
  onChange: (action: TriggerAction) => void;
}

function ActionFields({ action, rooms, contentItems, onChange }: ActionFieldsProps) {
  switch (action.type) {
    case 'send_message':
      return (
        <div className="space-y-2">
          <div>
            <label className="block text-xs text-gray-400 mb-1">대상</label>
            <select
              value={action.target}
              onChange={(e) => onChange({ ...action, target: e.target.value })}
              className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
            >
              <option value="player">플레이어만</option>
              <option value="room">방 전체</option>
            </select>
          </div>
          <div>
            <label className="block text-xs text-gray-400 mb-1">메시지</label>
            <textarea
              value={action.text}
              onChange={(e) => onChange({ ...action, text: e.target.value })}
              rows={2}
              placeholder="메시지 내용..."
              className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
            />
          </div>
        </div>
      );

    case 'spawn_entity':
      return (
        <div className="space-y-2">
          <div className="grid grid-cols-2 gap-2">
            <div>
              <label className="block text-xs text-gray-400 mb-1">유형</label>
              <select
                value={action.entity_type}
                onChange={(e) => onChange({ ...action, entity_type: e.target.value })}
                className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
              >
                <option value="npc">NPC</option>
                <option value="item">아이템</option>
              </select>
            </div>
            <div>
              <label className="block text-xs text-gray-400 mb-1">콘텐츠</label>
              <ContentSelect
                collection={action.entity_type === 'npc' ? 'monsters' : 'items'}
                value={action.content_id}
                contentItems={contentItems}
                onChange={(v) => onChange({ ...action, content_id: v })}
              />
            </div>
          </div>
          <div>
            <label className="block text-xs text-gray-400 mb-1">방</label>
            <RoomSelect rooms={rooms} value={action.room_id} onChange={(v) => onChange({ ...action, room_id: v })} />
          </div>
        </div>
      );

    case 'teleport':
      return (
        <div>
          <label className="block text-xs text-gray-400 mb-1">목적지 방</label>
          <RoomSelect rooms={rooms} value={action.room_id} onChange={(v) => onChange({ ...action, room_id: v })} />
        </div>
      );

    case 'give_item':
      return (
        <div>
          <label className="block text-xs text-gray-400 mb-1">아이템</label>
          <ContentSelect
            collection="items"
            value={action.content_id}
            contentItems={contentItems}
            onChange={(v) => onChange({ ...action, content_id: v })}
          />
        </div>
      );

    case 'heal':
      return (
        <div className="space-y-2">
          <div>
            <label className="block text-xs text-gray-400 mb-1">대상</label>
            <select
              value={action.target}
              onChange={(e) => onChange({ ...action, target: e.target.value })}
              className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
            >
              <option value="player">플레이어</option>
              <option value="entity">엔티티</option>
            </select>
          </div>
          <div>
            <label className="block text-xs text-gray-400 mb-1">회복 모드</label>
            <select
              value={action.mode}
              onChange={(e) => onChange({ ...action, mode: e.target.value })}
              className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
            >
              <option value="full">완전 회복 (최대 체력으로)</option>
              <option value="percent">비율 회복 (최대 체력의 N%)</option>
              <option value="fixed">고정량 회복 (N만큼)</option>
            </select>
          </div>
          {action.mode !== 'full' && (
            <div>
              <label className="block text-xs text-gray-400 mb-1">
                {action.mode === 'percent' ? '회복 비율 (%)' : '회복량'}
              </label>
              <input
                type="number"
                value={action.amount}
                onChange={(e) => onChange({ ...action, amount: Number(e.target.value) || 0 })}
                min={action.mode === 'percent' ? 1 : 1}
                max={action.mode === 'percent' ? 100 : undefined}
                placeholder={action.mode === 'percent' ? '1~100' : '회복할 HP'}
                className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
              />
              <p className="text-[10px] text-gray-500 mt-1">
                {action.mode === 'percent'
                  ? `최대 체력의 ${action.amount || 0}%를 회복합니다 (최대 체력 초과 불가)`
                  : `${action.amount || 0} HP를 회복합니다 (최대 체력 초과 불가)`}
              </p>
            </div>
          )}
        </div>
      );

    case 'set_component': {
      const compInfo = ECS_COMPONENTS.find((c) => c.value === action.component);
      const inputType = compInfo?.inputType || 'string';

      // Ensure value is the right shape when component changes
      const onCompChange = (comp: string) => {
        onChange({ ...action, component: comp, value: defaultComponentValue(comp) });
      };

      // Helper to update a nested key in an object value
      const val = action.value as Record<string, unknown> | undefined;
      const updateObj = (key: string, v: unknown) => {
        onChange({ ...action, value: { ...(typeof action.value === 'object' && action.value ? action.value as Record<string, unknown> : {}), [key]: v } });
      };

      return (
        <div className="space-y-2">
          <div>
            <label className="block text-xs text-gray-400 mb-1">컴포넌트</label>
            <select
              value={action.component}
              onChange={(e) => onCompChange(e.target.value)}
              className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
            >
              <option value="">-- 선택 --</option>
              {ECS_COMPONENTS.map((c) => (
                <option key={c.value} value={c.value}>{c.label}</option>
              ))}
            </select>
          </div>

          {/* Value input — varies by component type */}
          {inputType === 'number' && (
            <div>
              <label className="block text-xs text-gray-400 mb-1">값</label>
              <input
                type="number"
                value={typeof action.value === 'number' ? action.value : 0}
                onChange={(e) => onChange({ ...action, value: Number(e.target.value) || 0 })}
                className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
              />
            </div>
          )}

          {inputType === 'string' && (
            <div>
              <label className="block text-xs text-gray-400 mb-1">값</label>
              <input
                type="text"
                value={String(action.value ?? '')}
                onChange={(e) => onChange({ ...action, value: e.target.value })}
                placeholder="텍스트 값"
                className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
              />
            </div>
          )}

          {inputType === 'boolean' && (
            <p className="text-xs text-gray-500">
              이 컴포넌트를 엔티티에 설정합니다 (태그).
            </p>
          )}

          {inputType === 'health' && (
            <div className="grid grid-cols-2 gap-2">
              <div>
                <label className="block text-xs text-gray-400 mb-1">현재 체력</label>
                <input
                  type="number"
                  value={typeof val?.current === 'number' ? val.current : 0}
                  onChange={(e) => updateObj('current', Number(e.target.value) || 0)}
                  className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
                />
              </div>
              <div>
                <label className="block text-xs text-gray-400 mb-1">최대 체력</label>
                <input
                  type="number"
                  value={typeof val?.max === 'number' ? val.max : 0}
                  onChange={(e) => updateObj('max', Number(e.target.value) || 0)}
                  className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
                />
              </div>
            </div>
          )}

          {inputType === 'level' && (
            <div className="grid grid-cols-3 gap-2">
              <div>
                <label className="block text-xs text-gray-400 mb-1">레벨</label>
                <input
                  type="number"
                  value={typeof val?.level === 'number' ? val.level : 1}
                  onChange={(e) => updateObj('level', Number(e.target.value) || 1)}
                  className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
                />
              </div>
              <div>
                <label className="block text-xs text-gray-400 mb-1">경험치</label>
                <input
                  type="number"
                  value={typeof val?.exp === 'number' ? val.exp : 0}
                  onChange={(e) => updateObj('exp', Number(e.target.value) || 0)}
                  className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
                />
              </div>
              <div>
                <label className="block text-xs text-gray-400 mb-1">다음 레벨</label>
                <input
                  type="number"
                  value={typeof val?.exp_next === 'number' ? val.exp_next : 100}
                  onChange={(e) => updateObj('exp_next', Number(e.target.value) || 100)}
                  className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
                />
              </div>
            </div>
          )}

          {inputType === 'skills' && (
            <div>
              <label className="block text-xs text-gray-400 mb-1">보유 스킬</label>
              <div className="flex flex-wrap gap-1.5 mb-2 min-h-[28px]">
                {(Array.isArray((val as Record<string, unknown>)?.learned)
                  ? ((val as Record<string, unknown>).learned as string[])
                  : []
                ).map((skill, i) => (
                  <span
                    key={i}
                    className="inline-flex items-center gap-1 bg-gray-700 border border-gray-600 rounded px-2 py-0.5 text-xs"
                  >
                    {skill}
                    <button
                      onClick={() => {
                        const learned = [...((val as Record<string, unknown>)?.learned as string[] || [])];
                        learned.splice(i, 1);
                        onChange({ ...action, value: { learned } });
                      }}
                      className="text-gray-500 hover:text-red-400"
                    >&times;</button>
                  </span>
                ))}
              </div>
              {contentItems['skills'] && contentItems['skills'].length > 0 ? (
                <select
                  value=""
                  onChange={(e) => {
                    if (e.target.value) {
                      const learned = [...((val as Record<string, unknown>)?.learned as string[] || [])];
                      if (!learned.includes(e.target.value)) learned.push(e.target.value);
                      onChange({ ...action, value: { learned } });
                      e.target.value = '';
                    }
                  }}
                  className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm text-gray-400"
                >
                  <option value="">+ 스킬 추가...</option>
                  {contentItems['skills'].map((s) => (
                    <option key={s.id} value={s.id}>{String(s.name || s.id)}</option>
                  ))}
                </select>
              ) : (
                <input
                  type="text"
                  placeholder="스킬 ID 입력 후 Enter"
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') {
                      const v = (e.target as HTMLInputElement).value.trim();
                      if (v) {
                        const learned = [...((val as Record<string, unknown>)?.learned as string[] || [])];
                        if (!learned.includes(v)) learned.push(v);
                        onChange({ ...action, value: { learned } });
                        (e.target as HTMLInputElement).value = '';
                      }
                    }
                  }}
                  className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
                />
              )}
            </div>
          )}
        </div>
      );
    }

    case 'despawn_trigger_entity':
      return (
        <p className="text-xs text-gray-500">
          트리거를 발동시킨 엔티티를 월드에서 제거합니다.
        </p>
      );
  }
}

// --- Shared select components ---

function RoomSelect({
  rooms,
  value,
  onChange,
}: {
  rooms: Room[];
  value: string;
  onChange: (v: string) => void;
}) {
  return (
    <select
      value={value}
      onChange={(e) => onChange(e.target.value)}
      className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
    >
      <option value="">-- 방 선택 --</option>
      {rooms.map((r) => (
        <option key={r.id} value={r.name || r.id}>
          {r.name || r.id}
        </option>
      ))}
    </select>
  );
}

function ContentSelect({
  collection,
  value,
  contentItems,
  onChange,
}: {
  collection: string;
  value: string;
  contentItems: Record<string, ContentItem[]>;
  onChange: (v: string) => void;
}) {
  const items = contentItems[collection] || [];
  if (items.length === 0) {
    return (
      <input
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder="콘텐츠 ID"
        className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
      />
    );
  }
  return (
    <select
      value={value}
      onChange={(e) => onChange(e.target.value)}
      className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
    >
      <option value="">-- 선택 --</option>
      {items.map((item) => (
        <option key={item.id} value={String(item.name || item.id)}>
          {String(item.name || item.id)}
        </option>
      ))}
    </select>
  );
}
