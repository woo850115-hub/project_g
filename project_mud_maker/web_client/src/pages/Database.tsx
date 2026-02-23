import { useCallback, useEffect, useState } from 'react';
import { contentApi } from '../api/client';
import type { ContentItem } from '../types/content';
import { PromptDialog, ConfirmDialog } from '../components/Modal';
import { Tooltip } from '../components/Tooltip';

export function Database() {
  const [collections, setCollections] = useState<string[]>([]);
  const [activeCollection, setActiveCollection] = useState<string | null>(null);
  const [items, setItems] = useState<ContentItem[]>([]);
  const [activeItemId, setActiveItemId] = useState<string | null>(null);
  const [editData, setEditData] = useState<Record<string, unknown>>({});
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');

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
      setEditData({});
    } catch (e) {
      setError(e instanceof Error ? e.message : '\uC544\uC774\uD15C \uBD88\uB7EC\uC624\uAE30 \uC2E4\uD328');
    }
  }, [activeCollection]);

  useEffect(() => {
    loadCollections();
  }, [loadCollections]);

  useEffect(() => {
    loadItems();
  }, [loadItems]);

  // Select item for editing
  const selectItem = (item: ContentItem) => {
    setActiveItemId(item.id);
    setEditData({ ...item });
  };

  // Update a field
  const updateField = (key: string, value: unknown) => {
    setEditData((prev) => ({ ...prev, [key]: value }));
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
        setEditData({ ...updated });
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : '\uC800\uC7A5 \uC2E4\uD328');
    } finally {
      setSaving(false);
    }
  };

  // Add new item
  const handleAddItem = async (id: string) => {
    if (!activeCollection) return;
    setAddItemDialog(false);
    try {
      await contentApi.updateItem(activeCollection, id, { id } as ContentItem);
      await loadItems();
      setActiveItemId(id);
      setEditData({ id });
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
      setEditData({});
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
  const handleAddField = (key: string) => {
    if (key === 'id') return;
    setAddFieldDialog(false);
    setEditData((prev) => ({ ...prev, [key]: '' }));
  };

  // Remove a field
  const removeField = (key: string) => {
    if (key === 'id') return;
    setEditData((prev) => {
      const next = { ...prev };
      delete next[key];
      return next;
    });
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
      <PromptDialog
        open={addFieldDialog}
        title="필드 추가"
        label="필드 이름"
        placeholder="예: hp"
        onSubmit={handleAddField}
        onCancel={() => setAddFieldDialog(false)}
      />

      {/* Left sidebar — collections + items */}
      <div className="w-64 border-r border-gray-700 bg-gray-800 flex flex-col">
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
                    {c}
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
        {activeItemId ? (
          <>
            <div className="flex items-center justify-between mb-6">
              <h2 className="text-xl font-bold">
                {(editData.name as string) || activeItemId}
              </h2>
              <div className="flex gap-2">
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
              {Object.entries(editData).map(([key, value]) => (
                <div key={key} className="flex items-start gap-3">
                  <label className="w-32 text-sm text-gray-400 pt-2 text-right shrink-0">
                    {key}
                  </label>
                  {key === 'id' ? (
                    <input
                      type="text"
                      value={String(value ?? '')}
                      disabled
                      className="flex-1 bg-gray-700/50 text-gray-500 border border-gray-600 rounded px-3 py-1.5 text-sm"
                    />
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
              ))}

              <Tooltip text="이 아이템에 새로운 속성 필드를 추가합니다">
                <button
                  onClick={() => setAddFieldDialog(true)}
                  className="text-sm text-blue-400 hover:text-blue-300"
                >
                  + 필드 추가
                </button>
              </Tooltip>
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
