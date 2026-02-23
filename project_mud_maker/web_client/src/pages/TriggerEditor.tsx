import { useCallback, useEffect, useState } from 'react';
import { triggerApi, contentApi, worldApi } from '../api/client';
import type { Trigger, TriggerCondition, TriggerAction } from '../types/trigger';
import type { Room } from '../types/world';
import type { ContentItem } from '../types/content';
import { PromptDialog, ConfirmDialog } from '../components/Modal';

const CONDITION_TYPES = [
  { value: 'enter_room', label: 'Room Entry' },
  { value: 'command', label: 'Player Command' },
  { value: 'tick_interval', label: 'Timer (Tick Interval)' },
  { value: 'entity_death', label: 'Entity Death' },
  { value: 'on_connect', label: 'Player Connect' },
] as const;

const ACTION_TYPES = [
  { value: 'send_message', label: 'Send Message' },
  { value: 'spawn_entity', label: 'Spawn Entity' },
  { value: 'teleport', label: 'Teleport Player' },
  { value: 'give_item', label: 'Give Item' },
  { value: 'set_component', label: 'Set Component' },
  { value: 'despawn_trigger_entity', label: 'Despawn Entity' },
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
      setError(e instanceof Error ? e.message : 'Failed to load triggers');
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
      setError(`Trigger ID "${id}" already exists`);
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
      setError(e instanceof Error ? e.message : 'Save failed');
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
      setError(e instanceof Error ? e.message : 'Generate failed');
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
        title="New Trigger"
        label="Trigger name"
        placeholder="e.g. Dungeon Warning"
        onSubmit={handleCreate}
        onCancel={() => setCreateDialog(false)}
      />
      <ConfirmDialog
        open={deleteDialog}
        title="Delete Trigger"
        message={`Delete trigger "${selected?.name}"?`}
        confirmLabel="Delete"
        onConfirm={handleDelete}
        onCancel={() => setDeleteDialog(false)}
      />

      {/* Left sidebar — trigger list */}
      <div className="w-64 border-r border-gray-700 bg-gray-800 flex flex-col">
        <div className="p-3 border-b border-gray-700">
          <div className="flex items-center justify-between mb-2">
            <span className="text-sm font-medium text-gray-300">Triggers</span>
            <button
              onClick={() => setCreateDialog(true)}
              className="text-xs px-2 py-1 bg-blue-600 hover:bg-blue-500 rounded"
            >
              + New
            </button>
          </div>
          <div className="flex gap-1">
            <button
              onClick={saveTriggers}
              disabled={saving}
              className="flex-1 text-xs px-2 py-1 bg-green-700 hover:bg-green-600 disabled:opacity-50 rounded"
            >
              {saving ? 'Saving...' : 'Save'}
            </button>
            <button
              onClick={generateLua}
              className="flex-1 text-xs px-2 py-1 bg-purple-700 hover:bg-purple-600 rounded"
            >
              Generate
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
              No triggers yet
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
              <span className="text-sm text-gray-400">Generated Lua Preview</span>
              <button
                onClick={() => setLuaPreview(null)}
                className="text-xs text-gray-500 hover:text-gray-300"
              >
                Close
              </button>
            </div>
            <pre className="p-4 text-xs font-mono text-green-300 whitespace-pre-wrap">
              {luaPreview}
            </pre>
          </div>
        ) : (
          <div className="flex items-center justify-center h-full text-gray-500 text-sm">
            Select a trigger to edit, or create a new one
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
            Enabled
          </label>
        </div>
        <button
          onClick={onDelete}
          className="px-3 py-1 text-xs bg-red-700 hover:bg-red-600 rounded"
        >
          Delete
        </button>
      </div>

      {/* Name */}
      <div>
        <label className="block text-xs text-gray-400 mb-1">Name</label>
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
          <span className="text-xs font-bold text-yellow-400 bg-yellow-400/10 px-2 py-0.5 rounded">WHEN</span>
          <select
            value={trigger.condition.type}
            onChange={(e) => updateCondition(makeDefaultCondition(e.target.value))}
            className="bg-gray-700 border border-gray-600 rounded px-2 py-1 text-sm"
          >
            {CONDITION_TYPES.map((c) => (
              <option key={c.value} value={c.value}>{c.label}</option>
            ))}
          </select>
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
          <span className="text-xs font-bold text-green-400 bg-green-400/10 px-2 py-0.5 rounded">THEN</span>
          <button
            onClick={addAction}
            className="text-xs text-blue-400 hover:text-blue-300"
          >
            + Add Action
          </button>
        </div>

        {trigger.actions.map((action, i) => (
          <div key={i} className="bg-gray-800/50 border border-gray-700 rounded-lg p-4">
            <div className="flex items-center justify-between mb-3">
              <div className="flex items-center gap-2">
                <span className="text-xs text-gray-500">Action {i + 1}</span>
                <select
                  value={action.type}
                  onChange={(e) => updateAction(i, makeDefaultAction(e.target.value))}
                  className="bg-gray-700 border border-gray-600 rounded px-2 py-1 text-sm"
                >
                  {ACTION_TYPES.map((a) => (
                    <option key={a.value} value={a.value}>{a.label}</option>
                  ))}
                </select>
              </div>
              {trigger.actions.length > 1 && (
                <button
                  onClick={() => removeAction(i)}
                  className="text-gray-500 hover:text-red-400 text-xs"
                >
                  Remove
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
          <label className="block text-xs text-gray-400 mb-1">Room</label>
          <select
            value={condition.room_id}
            onChange={(e) => onChange({ ...condition, room_id: e.target.value })}
            className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
          >
            <option value="">-- Select room --</option>
            {rooms.map((r) => (
              <option key={r.id} value={r.name || r.id}>{r.name || r.id}</option>
            ))}
          </select>
        </div>
      );

    case 'command':
      return (
        <div>
          <label className="block text-xs text-gray-400 mb-1">Command</label>
          <input
            type="text"
            value={condition.command}
            onChange={(e) => onChange({ ...condition, command: e.target.value })}
            placeholder="e.g. pray, search, talk"
            className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
          />
        </div>
      );

    case 'tick_interval':
      return (
        <div>
          <label className="block text-xs text-gray-400 mb-1">
            Interval (ticks) — 20 ticks = 1 second
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
          <label className="block text-xs text-gray-400 mb-1">Entity (content ID, leave empty for any)</label>
          <select
            value={condition.content_id}
            onChange={(e) => onChange({ ...condition, content_id: e.target.value })}
            className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
          >
            <option value="">Any entity</option>
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
          Fires when a player connects to the server.
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
            <label className="block text-xs text-gray-400 mb-1">Target</label>
            <select
              value={action.target}
              onChange={(e) => onChange({ ...action, target: e.target.value })}
              className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
            >
              <option value="player">Player only</option>
              <option value="room">Entire room</option>
            </select>
          </div>
          <div>
            <label className="block text-xs text-gray-400 mb-1">Message</label>
            <textarea
              value={action.text}
              onChange={(e) => onChange({ ...action, text: e.target.value })}
              rows={2}
              placeholder="Message text..."
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
              <label className="block text-xs text-gray-400 mb-1">Type</label>
              <select
                value={action.entity_type}
                onChange={(e) => onChange({ ...action, entity_type: e.target.value })}
                className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
              >
                <option value="npc">NPC</option>
                <option value="item">Item</option>
              </select>
            </div>
            <div>
              <label className="block text-xs text-gray-400 mb-1">Content</label>
              <ContentSelect
                collection={action.entity_type === 'npc' ? 'monsters' : 'items'}
                value={action.content_id}
                contentItems={contentItems}
                onChange={(v) => onChange({ ...action, content_id: v })}
              />
            </div>
          </div>
          <div>
            <label className="block text-xs text-gray-400 mb-1">Room</label>
            <RoomSelect rooms={rooms} value={action.room_id} onChange={(v) => onChange({ ...action, room_id: v })} />
          </div>
        </div>
      );

    case 'teleport':
      return (
        <div>
          <label className="block text-xs text-gray-400 mb-1">Destination Room</label>
          <RoomSelect rooms={rooms} value={action.room_id} onChange={(v) => onChange({ ...action, room_id: v })} />
        </div>
      );

    case 'give_item':
      return (
        <div>
          <label className="block text-xs text-gray-400 mb-1">Item</label>
          <ContentSelect
            collection="items"
            value={action.content_id}
            contentItems={contentItems}
            onChange={(v) => onChange({ ...action, content_id: v })}
          />
        </div>
      );

    case 'set_component':
      return (
        <div className="space-y-2">
          <div>
            <label className="block text-xs text-gray-400 mb-1">Component</label>
            <input
              type="text"
              value={action.component}
              onChange={(e) => onChange({ ...action, component: e.target.value })}
              placeholder="e.g. Health, Attack"
              className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
            />
          </div>
          <div>
            <label className="block text-xs text-gray-400 mb-1">Value</label>
            <input
              type="text"
              value={String(action.value ?? '')}
              onChange={(e) => {
                const v = e.target.value;
                const num = Number(v);
                onChange({ ...action, value: !isNaN(num) && v !== '' ? num : v });
              }}
              placeholder="Value"
              className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
            />
          </div>
        </div>
      );

    case 'despawn_trigger_entity':
      return (
        <p className="text-xs text-gray-500">
          Removes the triggering entity from the world.
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
      <option value="">-- Select room --</option>
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
        placeholder="Content ID"
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
      <option value="">-- Select --</option>
      {items.map((item) => (
        <option key={item.id} value={String(item.name || item.id)}>
          {String(item.name || item.id)}
        </option>
      ))}
    </select>
  );
}
