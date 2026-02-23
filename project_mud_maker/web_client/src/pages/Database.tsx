import { useCallback, useEffect, useState } from 'react';
import { contentApi } from '../api/client';
import type { ContentItem } from '../types/content';
import { PromptDialog, ConfirmDialog } from '../components/Modal';

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
      setError(e instanceof Error ? e.message : 'Failed to load');
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
      setError(e instanceof Error ? e.message : 'Failed to load items');
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
      setError(e instanceof Error ? e.message : 'Save failed');
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
      setError(e instanceof Error ? e.message : 'Add failed');
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
      setError(e instanceof Error ? e.message : 'Delete failed');
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
      setError(e instanceof Error ? e.message : 'Create failed');
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
      setError(e instanceof Error ? e.message : 'Delete failed');
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
        title="Add Item"
        label="Item ID"
        placeholder="e.g. goblin_warrior"
        onSubmit={handleAddItem}
        onCancel={() => setAddItemDialog(false)}
      />
      <ConfirmDialog
        open={deleteItemDialog}
        title="Delete Item"
        message={`Delete "${activeItemId}" from ${activeCollection}?`}
        confirmLabel="Delete"
        onConfirm={handleDeleteItem}
        onCancel={() => setDeleteItemDialog(false)}
      />
      <PromptDialog
        open={addCollectionDialog}
        title="New Collection"
        label="Collection name"
        placeholder="e.g. monsters"
        onSubmit={handleAddCollection}
        onCancel={() => setAddCollectionDialog(false)}
      />
      <ConfirmDialog
        open={deleteCollectionDialog}
        title="Delete Collection"
        message={`Delete collection "${activeCollection}" and all its items?`}
        confirmLabel="Delete"
        onConfirm={handleDeleteCollection}
        onCancel={() => setDeleteCollectionDialog(false)}
      />
      <PromptDialog
        open={addFieldDialog}
        title="Add Field"
        label="Field name"
        placeholder="e.g. hp"
        onSubmit={handleAddField}
        onCancel={() => setAddFieldDialog(false)}
      />

      {/* Left sidebar — collections + items */}
      <div className="w-64 border-r border-gray-700 bg-gray-800 flex flex-col">
        {/* Collection selector */}
        <div className="p-3 border-b border-gray-700">
          <div className="flex items-center gap-2 mb-2">
            <select
              className="flex-1 bg-gray-700 text-sm rounded px-2 py-1.5 border border-gray-600"
              value={activeCollection || ''}
              onChange={(e) => {
                setActiveCollection(e.target.value || null);
                setSearchQuery('');
              }}
            >
              <option value="">-- Select --</option>
              {collections.map((c) => (
                <option key={c} value={c}>
                  {c}
                </option>
              ))}
            </select>
          </div>
          <div className="flex gap-1">
            <button
              onClick={() => setAddCollectionDialog(true)}
              className="text-xs px-2 py-1 bg-blue-600 hover:bg-blue-500 rounded"
            >
              + New
            </button>
            {activeCollection && (
              <button
                onClick={() => setDeleteCollectionDialog(true)}
                className="text-xs px-2 py-1 bg-red-700 hover:bg-red-600 rounded"
              >
                Delete
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
              placeholder="Search items..."
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
              {searchQuery ? 'No matching items' : 'No items yet'}
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
              + Add Item
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
                  {saving ? 'Saving...' : 'Save'}
                </button>
                <button
                  onClick={() => setDeleteItemDialog(true)}
                  className="px-4 py-1.5 text-sm bg-red-700 hover:bg-red-600 rounded"
                >
                  Delete
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
                      title="Remove field"
                    >
                      x
                    </button>
                  )}
                </div>
              ))}

              <button
                onClick={() => setAddFieldDialog(true)}
                className="text-sm text-blue-400 hover:text-blue-300"
              >
                + Add Field
              </button>
            </div>
          </>
        ) : (
          <div className="flex items-center justify-center h-full text-gray-500">
            <p>{activeCollection ? 'Select an item to edit' : 'Select a collection'}</p>
          </div>
        )}
      </div>
    </div>
  );
}
