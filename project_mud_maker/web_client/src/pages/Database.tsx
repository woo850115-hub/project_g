import { useCallback, useEffect, useMemo, useState } from 'react';
import { contentApi, itemEffectsApi, shopApi, attributeSchemaApi } from '../api/client';
import type { ContentItem } from '../types/content';
import type { AttributeSchema } from '../types/attribute_schema';
import { PromptDialog, ConfirmDialog, AddFieldDialog } from '../components/Modal';
import type { FieldPreset } from '../components/Modal';
import { Tooltip } from '../components/Tooltip';
import { BalanceView } from '../components/BalanceView';
import { useHistory } from '../hooks/useHistory';

// Korean labels for collection names
const COLLECTION_LABELS: Record<string, string> = {
  monsters: '몬스터',
  items: '아이템',
  races: '종족',
  classes: '직업',
  skills: '스킬',
  shops: '상점',
  quests: '퀘스트',
  dialogues: '대화',
  attribute_schema: '속성 스키마',
};

// Enum fields: collection → field → selectable options
const ENUM_OPTIONS: Record<string, Record<string, { value: string; label: string }[]>> = {
  items: {
    item_type: [
      { value: 'weapon', label: '무기 (weapon)' },
      { value: 'armor', label: '방어구 (armor)' },
      { value: 'consumable', label: '소모품 (consumable)' },
      { value: 'material', label: '재료 (material)' },
      { value: 'currency', label: '화폐 (currency)' },
      { value: 'quest', label: '퀘스트 (quest)' },
      { value: 'tool', label: '도구 (tool)' },
    ],
    equip_slot: [
      { value: 'weapon', label: '무기 (weapon)' },
      { value: 'armor', label: '방어구 (armor)' },
      { value: 'accessory', label: '장신구 (accessory)' },
    ],
  },
  skills: {
    type: [
      { value: 'attack', label: '공격 (attack)' },
      { value: 'heal', label: '치유 (heal)' },
      { value: 'attack_heal', label: '공격+치유 (attack_heal)' },
    ],
  },
};

// Fields that reference IDs from another collection
const REF_FIELDS: Record<string, Record<string, { refCollection: string; multiple: boolean }>> = {
  races: {
    racial_skill: { refCollection: 'skills', multiple: false },
  },
  classes: {
    starting_skills: { refCollection: 'skills', multiple: true },
  },
};

const FIELD_PRESETS: Record<string, FieldPreset[]> = {
  monsters: [
    { key: 'name', label: '이름', desc: '몬스터 표시 이름', type: 'string' },
    { key: 'description', label: '설명', desc: '몬스터 설명 텍스트', type: 'string' },
    { key: 'hp', label: '체력', desc: '최대 체력 (HP)', type: 'number' },
    { key: 'attack', label: '공격력', desc: '기본 공격력', type: 'number' },
    { key: 'defense', label: '방어력', desc: '기본 방어력', type: 'number' },
    { key: 'level', label: '레벨', desc: 'NPC 레벨 (Level 컴포넌트 설정)', type: 'number' },
    { key: 'gold', label: '소지 골드', desc: 'NPC가 보유한 골드', type: 'number' },
    { key: 'race', label: '종족', desc: 'NPC 종족 (Race 컴포넌트)', type: 'string' },
    { key: 'class', label: '직업', desc: 'NPC 직업 (Class 컴포넌트)', type: 'string' },
    { key: 'skills', label: '스킬', desc: 'NPC 보유 스킬 ID 목록 (배열)', type: 'array' },
  ],
  items: [
    { key: 'name', label: '이름', desc: '아이템 표시 이름', type: 'string' },
    { key: 'description', label: '설명', desc: '아이템 설명 텍스트', type: 'string' },
    { key: 'item_type', label: '아이템 유형', desc: '무기 / 방어구 / 소모품 / 재료 / 화폐 / 퀘스트 / 도구', type: 'string' },
    { key: 'value', label: '가치', desc: '골드 가치 또는 화폐 단위', type: 'number' },
    { key: 'attack_bonus', label: '공격력 보너스', desc: '장착 시 공격력 증가량', type: 'number' },
    { key: 'defense_bonus', label: '방어력 보너스', desc: '장착 시 방어력 증가량', type: 'number' },
    { key: 'heal_amount', label: '회복량', desc: '사용 시 HP 회복량', type: 'number' },
    { key: 'equip_slot', label: '장착 부위', desc: '장착 가능 부위 (weapon / armor / accessory)', type: 'string' },
    { key: 'use_message', label: '사용 메시지', desc: '소비 아이템 사용 시 표시할 메시지', type: 'string' },
  ],
  races: [
    { key: 'name', label: '이름', desc: '종족 표시 이름', type: 'string' },
    { key: 'description', label: '설명', desc: '종족 설명 텍스트', type: 'string' },
    { key: 'hp_bonus', label: 'HP 보너스', desc: '기본 체력 보정치', type: 'number' },
    { key: 'attack_bonus', label: '공격력 보너스', desc: '기본 공격력 보정치', type: 'number' },
    { key: 'defense_bonus', label: '방어력 보너스', desc: '기본 방어력 보정치', type: 'number' },
    { key: 'racial_skill', label: '종족 스킬', desc: '종족 고유 스킬 (없으면 비워두기)', type: 'string' },
  ],
  classes: [
    { key: 'name', label: '이름', desc: '직업 표시 이름', type: 'string' },
    { key: 'description', label: '설명', desc: '직업 설명 텍스트', type: 'string' },
    { key: 'hp_bonus', label: 'HP 보너스', desc: '기본 체력 보정치', type: 'number' },
    { key: 'attack_bonus', label: '공격력 보너스', desc: '기본 공격력 보정치', type: 'number' },
    { key: 'defense_bonus', label: '방어력 보너스', desc: '기본 방어력 보정치', type: 'number' },
    { key: 'starting_skills', label: '시작 스킬', desc: '캐릭터 생성 시 보유 스킬 목록 (배열)', type: 'array' },
  ],
  skills: [
    { key: 'name', label: '이름', desc: '스킬 표시 이름', type: 'string' },
    { key: 'description', label: '설명', desc: '스킬 설명 텍스트', type: 'string' },
    { key: 'type', label: '스킬 유형', desc: '공격 / 치유 / 공격+치유', type: 'string' },
    { key: 'damage_mult', label: '데미지 배율', desc: '기본 공격 대비 데미지 배율', type: 'number' },
    { key: 'heal_amount', label: '회복량', desc: '치유/공격+치유 스킬의 체력 회복량', type: 'number' },
    { key: 'cooldown', label: '쿨다운', desc: '사용 후 재사용 대기 틱 수', type: 'number' },
  ],
};

// Convert schema value_type to FieldPreset type
function schemaToPresetType(vt: string): 'string' | 'number' | 'boolean' | 'array' | 'object' {
  switch (vt) {
    case 'number': return 'number';
    case 'boolean': return 'boolean';
    case 'tags': return 'array';
    case 'range': return 'object';
    default: return 'string';
  }
}

// Get the default value for a schema attribute
function schemaDefaultValue(schema: AttributeSchema): unknown {
  switch (schema.value_type) {
    case 'number': return typeof schema.default === 'number' ? schema.default : 0;
    case 'string': return typeof schema.default === 'string' ? schema.default : '';
    case 'boolean': return typeof schema.default === 'boolean' ? schema.default : false;
    case 'range': return schema.default && typeof schema.default === 'object' ? schema.default : { current: 0, max: 0 };
    case 'select': return typeof schema.default === 'string' ? schema.default : '';
    case 'tags': return Array.isArray(schema.default) ? schema.default : [];
    default: return '';
  }
}

export function Database() {
  const [collections, setCollections] = useState<string[]>([]);
  const [activeCollection, setActiveCollection] = useState<string | null>(null);
  const [items, setItems] = useState<ContentItem[]>([]);
  const [activeItemId, setActiveItemId] = useState<string | null>(null);
  const editHistory = useHistory<Record<string, unknown>>({});
  const editData = editHistory.state;
  const setEditData = editHistory.set;
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [refItems, setRefItems] = useState<Record<string, ContentItem[]>>({});
  const [viewMode, setViewMode] = useState<'edit' | 'balance'>('edit');
  const [allCollectionItems, setAllCollectionItems] = useState<Record<string, ContentItem[]>>({});
  const [attrSchemas, setAttrSchemas] = useState<AttributeSchema[]>([]);

  // Dialog states
  const [addItemDialog, setAddItemDialog] = useState(false);
  const [deleteItemDialog, setDeleteItemDialog] = useState(false);
  const [addCollectionDialog, setAddCollectionDialog] = useState(false);
  const [deleteCollectionDialog, setDeleteCollectionDialog] = useState(false);
  const [addFieldDialog, setAddFieldDialog] = useState(false);

  // Load collections
  const loadCollections = useCallback(async () => {
    try {
      const cols = await contentApi.listCollections();
      setCollections(cols);
      if (cols.length > 0 && !activeCollection) {
        setActiveCollection(cols[0]);
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : '\uBD88\uB7EC\uC624\uAE30 \uC2E4\uD328');
    }
  }, [activeCollection]);

  // Load items when collection changes
  const loadItems = useCallback(async () => {
    if (!activeCollection) return;
    try {
      const data = await contentApi.listItems(activeCollection);
      setItems(data);
      setActiveItemId(null);
      editHistory.replace({});
    } catch (e) {
      setError(e instanceof Error ? e.message : '\uC544\uC774\uD15C \uBD88\uB7EC\uC624\uAE30 \uC2E4\uD328');
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeCollection]);

  // Load reference collections for cross-collection selectors + balance view
  const loadRefCollections = useCallback(async () => {
    const needed = new Set<string>();
    for (const refs of Object.values(REF_FIELDS)) {
      for (const r of Object.values(refs)) {
        needed.add(r.refCollection);
      }
    }
    const result: Record<string, ContentItem[]> = {};
    for (const col of needed) {
      try {
        result[col] = await contentApi.listItems(col);
      } catch { /* skip */ }
    }
    setRefItems(result);

    // Load all collection items for balance view
    try {
      const cols = await contentApi.listCollections();
      const all: Record<string, ContentItem[]> = {};
      for (const col of cols) {
        try {
          all[col] = await contentApi.listItems(col);
        } catch { /* skip */ }
      }
      setAllCollectionItems(all);
    } catch { /* skip */ }
  }, []);

  // Load attribute schemas
  const loadAttrSchemas = useCallback(async () => {
    try {
      const data = await attributeSchemaApi.list();
      setAttrSchemas(data);
    } catch { /* ignore */ }
  }, []);

  useEffect(() => {
    loadCollections();
    loadRefCollections();
    loadAttrSchemas();
  }, [loadCollections, loadRefCollections, loadAttrSchemas]);

  // Merge FIELD_PRESETS with attribute schemas for the active collection
  const mergedPresets = useMemo((): FieldPreset[] => {
    const base = activeCollection ? (FIELD_PRESETS[activeCollection] || []) : [];
    const schemaPresets: FieldPreset[] = attrSchemas
      .filter((s) => s.applies_to.length === 0 || (activeCollection && s.applies_to.includes(activeCollection)))
      .filter((s) => !base.some((p) => p.key === s.id))
      .map((s) => ({
        key: s.id,
        label: s.label,
        desc: s.description || `${s.label} (${s.value_type})`,
        type: schemaToPresetType(s.value_type),
      }));
    return [...base, ...schemaPresets];
  }, [activeCollection, attrSchemas]);

  useEffect(() => {
    loadItems();
  }, [loadItems]);

  // Select item for editing (replace = don't push to history on initial selection)
  const selectItem = (item: ContentItem) => {
    setActiveItemId(item.id);
    editHistory.replace({ ...item });
  };

  // Update a field
  const updateField = (key: string, value: unknown) => {
    setEditData({ ...editData, [key]: value });
  };

  // Save current item
  const saveItem = async () => {
    if (!activeCollection || !activeItemId) return;
    setSaving(true);
    try {
      await contentApi.updateItem(activeCollection, activeItemId, editData as ContentItem);
      await loadItems();
      setActiveItemId(activeItemId);
      const updated = (await contentApi.listItems(activeCollection)).find(
        (i) => i.id === activeItemId
      );
      if (updated) {
        editHistory.replace({ ...updated });
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : '\uC800\uC7A5 \uC2E4\uD328');
    } finally {
      setSaving(false);
    }
  };

  // Add new item (auto-populate FIELD_PRESETS + schema defaults)
  const handleAddItem = async (id: string) => {
    if (!activeCollection) return;
    setAddItemDialog(false);
    try {
      const initial: Record<string, unknown> = { id };
      // Apply base presets
      const presets = FIELD_PRESETS[activeCollection];
      if (presets) {
        for (const p of presets) {
          if (p.key === 'id') continue;
          initial[p.key] = p.type === 'number' ? 0
            : p.type === 'boolean' ? false
            : p.type === 'array' ? []
            : p.type === 'object' ? {}
            : '';
        }
      }
      // Apply schema defaults
      for (const schema of attrSchemas) {
        if (schema.applies_to.length > 0 && !schema.applies_to.includes(activeCollection)) continue;
        if (initial[schema.id] !== undefined) continue;
        initial[schema.id] = schemaDefaultValue(schema);
      }
      await contentApi.updateItem(activeCollection, id, initial as ContentItem);
      await loadItems();
      setActiveItemId(id);
      editHistory.replace(initial);
    } catch (e) {
      setError(e instanceof Error ? e.message : '\uCD94\uAC00 \uC2E4\uD328');
    }
  };

  // Delete item
  const handleDeleteItem = async () => {
    if (!activeCollection || !activeItemId) return;
    setDeleteItemDialog(false);
    try {
      await contentApi.deleteItem(activeCollection, activeItemId);
      setActiveItemId(null);
      editHistory.replace({});
      await loadItems();
    } catch (e) {
      setError(e instanceof Error ? e.message : '\uC0AD\uC81C \uC2E4\uD328');
    }
  };

  // Add new collection
  const handleAddCollection = async (id: string) => {
    setAddCollectionDialog(false);
    try {
      await contentApi.createCollection(id);
      await loadCollections();
      setActiveCollection(id);
    } catch (e) {
      setError(e instanceof Error ? e.message : '\uC0DD\uC131 \uC2E4\uD328');
    }
  };

  // Delete collection
  const handleDeleteCollection = async () => {
    if (!activeCollection) return;
    setDeleteCollectionDialog(false);
    try {
      await contentApi.deleteCollection(activeCollection);
      setActiveCollection(null);
      setItems([]);
      await loadCollections();
    } catch (e) {
      setError(e instanceof Error ? e.message : '\uC0AD\uC81C \uC2E4\uD328');
    }
  };

  // Add a new field to the editing item
  const handleAddField = (key: string, type: 'string' | 'number' | 'boolean' | 'array' | 'object') => {
    if (key === 'id') return;
    setAddFieldDialog(false);
    const defaultValue = type === 'number' ? 0
      : type === 'boolean' ? false
      : type === 'array' ? []
      : type === 'object' ? {}
      : '';
    setEditData({ ...editData, [key]: defaultValue });
  };

  // Remove a field
  const removeField = (key: string) => {
    if (key === 'id') return;
    const next = { ...editData };
    delete next[key];
    setEditData(next);
  };

  // Filter items by search
  const filteredItems = items.filter((item) => {
    if (!searchQuery) return true;
    const q = searchQuery.toLowerCase();
    return (
      item.id.toLowerCase().includes(q) ||
      (typeof item.name === 'string' && item.name.toLowerCase().includes(q))
    );
  });

  // Generate item effects Lua
  const generateItemEffects = async () => {
    try {
      await itemEffectsApi.generate();
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : '아이템 효과 Lua 생성 실패');
    }
  };

  // Generate shop Lua
  const generateShopLua = async () => {
    try {
      await shopApi.generate();
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : '상점 Lua 생성 실패');
    }
  };

  // Handle switching from balance to edit with item selection
  const handleBalanceSelect = (collection: string, itemId: string) => {
    setViewMode('edit');
    setActiveCollection(collection);
    // Items will load from the collection change; queue item selection
    setTimeout(async () => {
      try {
        const colItems = await contentApi.listItems(collection);
        setItems(colItems);
        const item = colItems.find((i) => i.id === itemId);
        if (item) {
          setActiveItemId(item.id);
          editHistory.replace({ ...item });
        }
      } catch { /* ignore */ }
    }, 100);
  };

  // Balance view mode
  if (viewMode === 'balance') {
    return (
      <div className="flex flex-col h-full">
        <div className="flex items-center gap-2 px-4 py-2 border-b border-gray-700 bg-gray-800">
          <button
            onClick={() => setViewMode('edit')}
            className="px-3 py-1 text-xs bg-gray-600 hover:bg-gray-500 rounded"
          >
            편집
          </button>
          <button
            className="px-3 py-1 text-xs bg-blue-600 rounded"
          >
            밸런스
          </button>
        </div>
        <BalanceView collections={allCollectionItems} onSelectItem={handleBalanceSelect} />
      </div>
    );
  }

  return (
    <div className="flex h-full">
      {/* Error toast */}
      {error && (
        <div className="fixed top-4 right-4 bg-red-600 text-white px-4 py-2 rounded shadow-lg z-50">
          {error}
          <button className="ml-2 font-bold" onClick={() => setError(null)}>
            x
          </button>
        </div>
      )}

      {/* Dialogs */}
      <PromptDialog
        open={addItemDialog}
        title="아이템 추가"
        label="아이템 ID"
        placeholder="예: goblin_warrior"
        onSubmit={handleAddItem}
        onCancel={() => setAddItemDialog(false)}
      />
      <ConfirmDialog
        open={deleteItemDialog}
        title="아이템 삭제"
        message={`${activeCollection}에서 "${activeItemId}"을(를) 삭제하시겠습니까?`}
        confirmLabel="삭제"
        onConfirm={handleDeleteItem}
        onCancel={() => setDeleteItemDialog(false)}
      />
      <PromptDialog
        open={addCollectionDialog}
        title="새 컬렉션"
        label="컬렉션 이름"
        placeholder="예: monsters"
        onSubmit={handleAddCollection}
        onCancel={() => setAddCollectionDialog(false)}
      />
      <ConfirmDialog
        open={deleteCollectionDialog}
        title="컬렉션 삭제"
        message={`"${activeCollection}" 컬렉션과 모든 아이템을 삭제하시겠습니까?`}
        confirmLabel="삭제"
        onConfirm={handleDeleteCollection}
        onCancel={() => setDeleteCollectionDialog(false)}
      />
      <AddFieldDialog
        open={addFieldDialog}
        presets={mergedPresets}
        existingKeys={Object.keys(editData)}
        onSelect={handleAddField}
        onCancel={() => setAddFieldDialog(false)}
      />

      {/* Left sidebar — collections + items */}
      <div className="w-64 border-r border-gray-700 bg-gray-800 flex flex-col">
        {/* View mode toggle */}
        <div className="flex items-center gap-1 px-3 pt-2">
          <button
            onClick={() => setViewMode('edit')}
            className="px-2 py-0.5 text-xs bg-blue-600 rounded"
          >
            편집
          </button>
          <button
            onClick={() => { loadRefCollections(); setViewMode('balance'); }}
            className="px-2 py-0.5 text-xs bg-gray-600 hover:bg-gray-500 rounded"
          >
            밸런스
          </button>
        </div>

        {/* Collection selector */}
        <div className="p-3 border-b border-gray-700">
          <div className="flex items-center gap-2 mb-2">
            <Tooltip text="게임 콘텐츠 종류를 선택합니다 (예: monsters, items)">
              <select
                className="flex-1 bg-gray-700 text-sm rounded px-2 py-1.5 border border-gray-600"
                value={activeCollection || ''}
                onChange={(e) => {
                  setActiveCollection(e.target.value || null);
                  setSearchQuery('');
                }}
              >
                <option value="">-- 선택 --</option>
                {collections.map((c) => (
                  <option key={c} value={c}>
                    {COLLECTION_LABELS[c] || c}
                  </option>
                ))}
              </select>
            </Tooltip>
          </div>
          <div className="flex gap-1">
            <button
              onClick={() => setAddCollectionDialog(true)}
              className="text-xs px-2 py-1 bg-blue-600 hover:bg-blue-500 rounded"
            >
              + 새로 만들기
            </button>
            {activeCollection && (
              <button
                onClick={() => setDeleteCollectionDialog(true)}
                className="text-xs px-2 py-1 bg-red-700 hover:bg-red-600 rounded"
              >
                삭제
              </button>
            )}
          </div>
        </div>

        {/* Search bar */}
        {activeCollection && (
          <div className="px-3 py-2 border-b border-gray-700">
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="아이템 검색..."
              className="w-full bg-gray-700 border border-gray-600 rounded px-2 py-1 text-xs"
            />
          </div>
        )}

        {/* Item list */}
        <div className="flex-1 overflow-y-auto">
          {filteredItems.map((item) => (
            <button
              key={item.id}
              onClick={() => selectItem(item)}
              className={`w-full text-left px-3 py-2 text-sm border-b border-gray-700 transition-colors ${
                activeItemId === item.id
                  ? 'bg-blue-900/40 text-blue-300'
                  : 'hover:bg-gray-700/50'
              }`}
            >
              <div className="truncate">{(item.name as string) || item.id}</div>
              {typeof item.name === 'string' && item.name && (
                <div className="text-[10px] text-gray-500 truncate">{item.id}</div>
              )}
            </button>
          ))}
          {activeCollection && filteredItems.length === 0 && (
            <div className="p-3 text-xs text-gray-500 text-center">
              {searchQuery ? '검색 결과 없음' : '아이템이 없습니다'}
            </div>
          )}
        </div>

        {/* Add item button */}
        {activeCollection && (
          <div className="p-2 border-t border-gray-700">
            <button
              onClick={() => setAddItemDialog(true)}
              className="w-full text-xs px-2 py-1.5 bg-green-700 hover:bg-green-600 rounded"
            >
              + 아이템 추가
            </button>
          </div>
        )}
      </div>

      {/* Right panel — item editor */}
      <div className="flex-1 overflow-y-auto p-6">
        {activeItemId && activeCollection === 'shops' ? (
          /* ===== Shop dedicated editor ===== */
          <ShopEditor
            editData={editData}
            updateField={updateField}
            availableItems={allCollectionItems['items'] || []}
            saving={saving}
            onSave={saveItem}
            onDelete={() => setDeleteItemDialog(true)}
            onGenerateLua={generateShopLua}
            editHistory={editHistory}
          />
        ) : activeItemId ? (
          <>
            <div className="flex items-center justify-between mb-6">
              <h2 className="text-xl font-bold">
                {(editData.name as string) || activeItemId}
              </h2>
              <div className="flex gap-2">
                <button
                  onClick={editHistory.undo}
                  disabled={!editHistory.canUndo}
                  className="px-3 py-1.5 text-sm bg-gray-600 hover:bg-gray-500 disabled:opacity-30 rounded"
                  title="실행취소 (Ctrl+Z)"
                >
                  실행취소
                </button>
                <button
                  onClick={editHistory.redo}
                  disabled={!editHistory.canRedo}
                  className="px-3 py-1.5 text-sm bg-gray-600 hover:bg-gray-500 disabled:opacity-30 rounded"
                  title="다시실행 (Ctrl+Y)"
                >
                  다시실행
                </button>
                <button
                  onClick={saveItem}
                  disabled={saving}
                  className="px-4 py-1.5 text-sm bg-blue-600 hover:bg-blue-500 disabled:opacity-50 rounded"
                >
                  {saving ? '저장 중...' : '저장'}
                </button>
                <button
                  onClick={() => setDeleteItemDialog(true)}
                  className="px-4 py-1.5 text-sm bg-red-700 hover:bg-red-600 rounded"
                >
                  삭제
                </button>
              </div>
            </div>

            {/* Dynamic form */}
            <div className="space-y-4 max-w-2xl">
              {Object.entries(editData).map(([key, value]) => {
                const enumOpts = activeCollection ? ENUM_OPTIONS[activeCollection]?.[key] : undefined;
                const refField = activeCollection ? REF_FIELDS[activeCollection]?.[key] : undefined;
                const presetInfo = mergedPresets.find((p) => p.key === key);
                const fieldLabel = presetInfo ? `${key}` : key;
                const fieldHint = presetInfo?.desc;

                return (
                <div key={key} className="flex items-start gap-3">
                  <Tooltip text={fieldHint || key}>
                    <label className="w-32 text-sm text-gray-400 pt-2 text-right shrink-0">
                      {fieldLabel}
                    </label>
                  </Tooltip>
                  {key === 'id' ? (
                    <input
                      type="text"
                      value={String(value ?? '')}
                      disabled
                      className="flex-1 bg-gray-700/50 text-gray-500 border border-gray-600 rounded px-3 py-1.5 text-sm"
                    />
                  ) : enumOpts ? (
                    /* Enum field — select dropdown */
                    <select
                      value={String(value ?? '')}
                      onChange={(e) => updateField(key, e.target.value)}
                      className="flex-1 bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
                    >
                      <option value="">-- 선택 --</option>
                      {enumOpts.map((opt) => (
                        <option key={opt.value} value={opt.value}>{opt.label}</option>
                      ))}
                    </select>
                  ) : refField && !refField.multiple ? (
                    /* Single reference — select from other collection */
                    <select
                      value={String(value ?? '')}
                      onChange={(e) => updateField(key, e.target.value || null)}
                      className="flex-1 bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
                    >
                      <option value="">없음</option>
                      {(refItems[refField.refCollection] || []).map((item) => (
                        <option key={item.id} value={item.id}>
                          {(item.name as string) || item.id}
                        </option>
                      ))}
                    </select>
                  ) : refField && refField.multiple && Array.isArray(value) ? (
                    /* Multiple reference — tag list + add dropdown */
                    <div className="flex-1">
                      <div className="flex flex-wrap gap-1.5 mb-2 min-h-[28px]">
                        {(value as string[]).map((refId, i) => {
                          const refItem = (refItems[refField.refCollection] || []).find((r) => r.id === refId);
                          const display = refItem ? ((refItem.name as string) || refItem.id) : refId;
                          return (
                            <span
                              key={i}
                              className="inline-flex items-center gap-1 bg-gray-700 border border-gray-600 rounded px-2 py-0.5 text-xs"
                            >
                              {display}
                              <button
                                onClick={() => {
                                  const next = [...(value as string[])];
                                  next.splice(i, 1);
                                  updateField(key, next);
                                }}
                                className="text-gray-500 hover:text-red-400"
                              >
                                &times;
                              </button>
                            </span>
                          );
                        })}
                      </div>
                      <select
                        value=""
                        onChange={(e) => {
                          if (e.target.value) {
                            updateField(key, [...(value as string[]), e.target.value]);
                            e.target.value = '';
                          }
                        }}
                        className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm text-gray-400"
                      >
                        <option value="">+ 추가...</option>
                        {(refItems[refField.refCollection] || [])
                          .filter((item) => !(value as string[]).includes(item.id))
                          .map((item) => (
                            <option key={item.id} value={item.id}>
                              {(item.name as string) || item.id}
                            </option>
                          ))}
                      </select>
                    </div>
                  ) : typeof value === 'number' ? (
                    <input
                      type="number"
                      value={value}
                      onChange={(e) =>
                        updateField(key, e.target.value === '' ? '' : Number(e.target.value))
                      }
                      className="flex-1 bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
                    />
                  ) : typeof value === 'boolean' ? (
                    <input
                      type="checkbox"
                      checked={value}
                      onChange={(e) => updateField(key, e.target.checked)}
                      className="mt-2"
                    />
                  ) : typeof value === 'object' && value !== null ? (
                    <textarea
                      value={JSON.stringify(value, null, 2)}
                      onChange={(e) => {
                        try {
                          updateField(key, JSON.parse(e.target.value));
                        } catch {
                          // Allow intermediate invalid JSON while typing
                        }
                      }}
                      rows={4}
                      className="flex-1 bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm font-mono"
                    />
                  ) : (
                    <input
                      type="text"
                      value={String(value ?? '')}
                      onChange={(e) => updateField(key, e.target.value)}
                      className="flex-1 bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
                    />
                  )}
                  {key !== 'id' && (
                    <button
                      onClick={() => removeField(key)}
                      className="text-gray-500 hover:text-red-400 text-sm pt-2"
                      title="필드 제거"
                    >
                      x
                    </button>
                  )}
                </div>
                );
              })}

              <Tooltip text="이 아이템에 새로운 속성 필드를 추가합니다">
                <button
                  onClick={() => setAddFieldDialog(true)}
                  className="text-sm text-blue-400 hover:text-blue-300"
                >
                  + 필드 추가
                </button>
              </Tooltip>

              {/* Item effects preview for items collection */}
              {activeCollection === 'items' && typeof editData.item_type === 'string' && (
                <div className="mt-6 bg-gray-800/50 border border-gray-700 rounded-lg p-4">
                  <div className="flex items-center justify-between mb-2">
                    <span className="text-xs font-bold text-purple-400">효과 미리보기</span>
                    <button
                      onClick={generateItemEffects}
                      className="text-xs px-2 py-1 bg-purple-700 hover:bg-purple-600 rounded"
                    >
                      효과 Lua 생성
                    </button>
                  </div>
                  <div className="text-xs text-gray-400 space-y-1">
                    {editData.item_type === 'consumable' && (
                      <p>사용 시 {(editData.heal_amount as number) || 0} HP 회복</p>
                    )}
                    {editData.item_type === 'weapon' && (
                      <p>장착 시 공격력 +{(editData.attack_bonus as number) || 0}</p>
                    )}
                    {editData.item_type === 'armor' && (
                      <p>장착 시 방어력 +{(editData.defense_bonus as number) || 0}</p>
                    )}
                  </div>
                </div>
              )}
            </div>
          </>
        ) : (
          <div className="flex items-center justify-center h-full text-gray-500">
            <p>{activeCollection ? '편집할 아이템을 선택하세요' : '컬렉션을 선택하세요'}</p>
          </div>
        )}
      </div>
    </div>
  );
}

// ===== Shop Dedicated Editor =====

interface ShopEditorProps {
  editData: Record<string, unknown>;
  updateField: (key: string, value: unknown) => void;
  availableItems: ContentItem[];
  saving: boolean;
  onSave: () => void;
  onDelete: () => void;
  onGenerateLua: () => void;
  editHistory: import('../hooks/useHistory').HistoryControls<Record<string, unknown>>;
}

function ShopEditor({ editData, updateField, availableItems, saving, onSave, onDelete, onGenerateLua, editHistory }: ShopEditorProps) {
  const shopItems = (Array.isArray(editData.items) ? editData.items : []) as { item_id: string; price: number }[];
  const sellRate = typeof editData.sell_rate === 'number' ? editData.sell_rate : 0.5;

  const updateShopItems = (newItems: { item_id: string; price: number }[]) => {
    updateField('items', newItems);
  };

  const addShopItem = () => {
    updateShopItems([...shopItems, { item_id: '', price: 0 }]);
  };

  const removeShopItem = (index: number) => {
    updateShopItems(shopItems.filter((_, i) => i !== index));
  };

  const updateShopItem = (index: number, patch: Partial<{ item_id: string; price: number }>) => {
    const updated = [...shopItems];
    updated[index] = { ...updated[index], ...patch };
    updateShopItems(updated);
  };

  return (
    <div className="max-w-2xl space-y-6">
      <div className="flex items-center justify-between">
        <h2 className="text-xl font-bold">
          {(editData.name as string) || (editData.id as string) || '상점'}
        </h2>
        <div className="flex gap-2">
          <button
            onClick={editHistory.undo}
            disabled={!editHistory.canUndo}
            className="px-3 py-1.5 text-sm bg-gray-600 hover:bg-gray-500 disabled:opacity-30 rounded"
          >
            실행취소
          </button>
          <button
            onClick={editHistory.redo}
            disabled={!editHistory.canRedo}
            className="px-3 py-1.5 text-sm bg-gray-600 hover:bg-gray-500 disabled:opacity-30 rounded"
          >
            다시실행
          </button>
          <button onClick={onSave} disabled={saving} className="px-4 py-1.5 text-sm bg-blue-600 hover:bg-blue-500 disabled:opacity-50 rounded">
            {saving ? '저장 중...' : '저장'}
          </button>
          <button onClick={onDelete} className="px-4 py-1.5 text-sm bg-red-700 hover:bg-red-600 rounded">
            삭제
          </button>
        </div>
      </div>

      {/* Basic info */}
      <div className="grid grid-cols-2 gap-4">
        <div>
          <label className="block text-xs text-gray-400 mb-1">상점 이름</label>
          <input
            type="text"
            value={(editData.name as string) || ''}
            onChange={(e) => updateField('name', e.target.value)}
            placeholder="예: 마을 잡화점"
            className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
          />
        </div>
        <div>
          <label className="block text-xs text-gray-400 mb-1">NPC 이름</label>
          <input
            type="text"
            value={(editData.npc_name as string) || ''}
            onChange={(e) => updateField('npc_name', e.target.value)}
            placeholder="예: 상인 아저씨"
            className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
          />
        </div>
      </div>

      <div>
        <label className="block text-xs text-gray-400 mb-1">위치 (방 이름)</label>
        <input
          type="text"
          value={(editData.room_name as string) || ''}
          onChange={(e) => updateField('room_name', e.target.value)}
          placeholder="예: 마을 광장"
          className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
        />
      </div>

      {/* Sell rate slider */}
      <div>
        <label className="block text-xs text-gray-400 mb-1">
          매각 비율: {Math.round(sellRate * 100)}%
        </label>
        <input
          type="range"
          min={0}
          max={100}
          value={Math.round(sellRate * 100)}
          onChange={(e) => updateField('sell_rate', Number(e.target.value) / 100)}
          className="w-full"
        />
        <p className="text-[10px] text-gray-500 mt-1">
          플레이어가 아이템을 팔 때 원래 가격의 {Math.round(sellRate * 100)}%를 받습니다
        </p>
      </div>

      {/* Shop items table */}
      <div className="bg-gray-800/50 border border-gray-700 rounded-lg p-4 space-y-3">
        <div className="flex items-center justify-between">
          <span className="text-xs font-bold text-yellow-400 bg-yellow-400/10 px-2 py-0.5 rounded">
            판매 상품 ({shopItems.length}개)
          </span>
          <button onClick={addShopItem} className="text-xs text-blue-400 hover:text-blue-300">
            + 상품 추가
          </button>
        </div>

        {shopItems.length === 0 && (
          <p className="text-xs text-gray-500">판매 상품이 없습니다. 상품을 추가하세요.</p>
        )}

        {/* Table header */}
        {shopItems.length > 0 && (
          <div className="grid grid-cols-[1fr_100px_40px] gap-2 text-xs text-gray-500 border-b border-gray-700 pb-1">
            <span>아이템</span>
            <span>가격 (골드)</span>
            <span></span>
          </div>
        )}

        {shopItems.map((si, i) => (
          <div key={i} className="grid grid-cols-[1fr_100px_40px] gap-2 items-center">
            {availableItems.length > 0 ? (
              <select
                value={si.item_id}
                onChange={(e) => updateShopItem(i, { item_id: e.target.value })}
                className="bg-gray-700 border border-gray-600 rounded px-2 py-1.5 text-sm"
              >
                <option value="">-- 아이템 선택 --</option>
                {availableItems.map((item) => (
                  <option key={item.id} value={item.id}>
                    {(item.name as string) || item.id}
                  </option>
                ))}
              </select>
            ) : (
              <input
                type="text"
                value={si.item_id}
                onChange={(e) => updateShopItem(i, { item_id: e.target.value })}
                placeholder="아이템 ID"
                className="bg-gray-700 border border-gray-600 rounded px-2 py-1.5 text-sm"
              />
            )}
            <input
              type="number"
              value={si.price}
              onChange={(e) => updateShopItem(i, { price: Number(e.target.value) || 0 })}
              min={0}
              className="bg-gray-700 border border-gray-600 rounded px-2 py-1.5 text-sm"
            />
            <button
              onClick={() => removeShopItem(i)}
              className="text-gray-500 hover:text-red-400 text-sm"
            >
              x
            </button>
          </div>
        ))}
      </div>

      {/* Generate Lua */}
      <button
        onClick={onGenerateLua}
        className="px-4 py-2 text-sm bg-purple-700 hover:bg-purple-600 rounded"
      >
        상점 Lua 생성
      </button>
    </div>
  );
}
