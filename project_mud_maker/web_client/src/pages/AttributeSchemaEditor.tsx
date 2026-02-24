import { useCallback, useEffect, useState } from 'react';
import { attributeSchemaApi, contentApi } from '../api/client';
import type { AttributeSchema, AttributeValueType, SelectOption } from '../types/attribute_schema';

const VALUE_TYPE_LABELS: Record<AttributeValueType, string> = {
  number: '숫자',
  string: '문자열',
  boolean: '불리언',
  range: '범위 (current/max)',
  select: '선택',
  tags: '태그 (문자열 배열)',
};

const VALUE_TYPES: AttributeValueType[] = ['number', 'string', 'boolean', 'range', 'select', 'tags'];

function makeDefaultForType(type: AttributeValueType): unknown {
  switch (type) {
    case 'number': return 0;
    case 'string': return '';
    case 'boolean': return false;
    case 'range': return { current: 0, max: 0 };
    case 'select': return '';
    case 'tags': return [];
  }
}

function newSchema(): AttributeSchema {
  return {
    id: '',
    label: '',
    description: '',
    category: '',
    value_type: 'number',
    default: 0,
    applies_to: [],
    options: [],
  };
}

export function AttributeSchemaEditor() {
  const [schemas, setSchemas] = useState<AttributeSchema[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [editSchema, setEditSchema] = useState<AttributeSchema>(newSchema());
  const [collections, setCollections] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [dirty, setDirty] = useState(false);

  const loadSchemas = useCallback(async () => {
    try {
      const data = await attributeSchemaApi.list();
      setSchemas(data);
    } catch (e) {
      setError(e instanceof Error ? e.message : '스키마 로드 실패');
    }
  }, []);

  const loadCollections = useCallback(async () => {
    try {
      const cols = await contentApi.listCollections();
      setCollections(cols);
    } catch { /* ignore */ }
  }, []);

  useEffect(() => {
    loadSchemas();
    loadCollections();
  }, [loadSchemas, loadCollections]);

  const selectSchema = (schema: AttributeSchema) => {
    setSelectedId(schema.id);
    setEditSchema({ ...schema, options: [...(schema.options || [])] });
  };

  const handleAdd = () => {
    const s = newSchema();
    s.id = `attr_${Date.now()}`;
    setSchemas([...schemas, s]);
    selectSchema(s);
    setDirty(true);
  };

  const handleDelete = () => {
    if (!selectedId) return;
    setSchemas(schemas.filter((s) => s.id !== selectedId));
    setSelectedId(null);
    setEditSchema(newSchema());
    setDirty(true);
  };

  const updateEdit = (patch: Partial<AttributeSchema>) => {
    const updated = { ...editSchema, ...patch };
    setEditSchema(updated);
    // Also update in the list
    setSchemas(schemas.map((s) => (s.id === selectedId ? updated : s)));
    setDirty(true);
  };

  const handleValueTypeChange = (type: AttributeValueType) => {
    updateEdit({
      value_type: type,
      default: makeDefaultForType(type),
      options: type === 'select' ? (editSchema.options.length > 0 ? editSchema.options : []) : [],
    });
  };

  const handleSaveAll = async () => {
    setSaving(true);
    try {
      await attributeSchemaApi.save(schemas);
      setDirty(false);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : '저장 실패');
    } finally {
      setSaving(false);
    }
  };

  const toggleAppliesTo = (col: string) => {
    const current = editSchema.applies_to;
    const next = current.includes(col)
      ? current.filter((c) => c !== col)
      : [...current, col];
    updateEdit({ applies_to: next });
  };

  // Group schemas by category
  const categories = Array.from(new Set(schemas.map((s) => s.category || '기타'))).sort();
  const schemasByCategory: Record<string, AttributeSchema[]> = {};
  for (const cat of categories) {
    schemasByCategory[cat] = schemas.filter((s) => (s.category || '기타') === cat);
  }

  return (
    <div className="flex h-full">
      {/* Error toast */}
      {error && (
        <div className="fixed top-4 right-4 bg-red-600 text-white px-4 py-2 rounded shadow-lg z-50">
          {error}
          <button className="ml-2 font-bold" onClick={() => setError(null)}>x</button>
        </div>
      )}

      {/* Left panel — schema list */}
      <div className="w-64 border-r border-gray-700 bg-gray-800 flex flex-col">
        <div className="p-3 border-b border-gray-700">
          <button
            onClick={handleAdd}
            className="w-full text-xs px-2 py-1.5 bg-blue-600 hover:bg-blue-500 rounded"
          >
            + 속성 추가
          </button>
        </div>

        <div className="flex-1 overflow-y-auto">
          {categories.map((cat) => (
            <div key={cat}>
              <div className="px-3 py-1.5 text-[10px] text-gray-500 font-bold uppercase tracking-wider bg-gray-800/80 border-b border-gray-700/50">
                {cat}
              </div>
              {schemasByCategory[cat].map((schema) => (
                <button
                  key={schema.id}
                  onClick={() => selectSchema(schema)}
                  className={`w-full text-left px-3 py-2 text-sm border-b border-gray-700 transition-colors ${
                    selectedId === schema.id
                      ? 'bg-blue-900/40 text-blue-300'
                      : 'hover:bg-gray-700/50'
                  }`}
                >
                  <div className="truncate">{schema.label || schema.id}</div>
                  <div className="text-[10px] text-gray-500 truncate">{schema.id}</div>
                </button>
              ))}
            </div>
          ))}
          {schemas.length === 0 && (
            <div className="p-3 text-xs text-gray-500 text-center">
              속성이 없습니다
            </div>
          )}
        </div>

        <div className="p-2 border-t border-gray-700">
          <button
            onClick={handleSaveAll}
            disabled={saving || !dirty}
            className="w-full text-xs px-2 py-1.5 bg-green-700 hover:bg-green-600 disabled:opacity-50 rounded"
          >
            {saving ? '저장 중...' : '전체 저장'}
          </button>
        </div>
      </div>

      {/* Right panel — editor */}
      <div className="flex-1 overflow-y-auto p-6">
        {selectedId ? (
          <div className="max-w-2xl space-y-4">
            <div className="flex items-center justify-between mb-6">
              <h2 className="text-xl font-bold">{editSchema.label || editSchema.id}</h2>
              <div className="flex gap-2">
                <button
                  onClick={handleSaveAll}
                  disabled={saving || !dirty}
                  className="px-4 py-1.5 text-sm bg-blue-600 hover:bg-blue-500 disabled:opacity-50 rounded"
                >
                  {saving ? '저장 중...' : '저장'}
                </button>
                <button
                  onClick={handleDelete}
                  className="px-4 py-1.5 text-sm bg-red-700 hover:bg-red-600 rounded"
                >
                  삭제
                </button>
              </div>
            </div>

            {/* ID */}
            <div className="flex items-start gap-3">
              <label className="w-24 text-sm text-gray-400 pt-2 text-right shrink-0">ID</label>
              <input
                type="text"
                value={editSchema.id}
                onChange={(e) => {
                  const newId = e.target.value;
                  setSchemas(schemas.map((s) => (s.id === selectedId ? { ...editSchema, id: newId } : s)));
                  setSelectedId(newId);
                  setEditSchema({ ...editSchema, id: newId });
                  setDirty(true);
                }}
                className="flex-1 bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
              />
            </div>

            {/* Label */}
            <div className="flex items-start gap-3">
              <label className="w-24 text-sm text-gray-400 pt-2 text-right shrink-0">이름</label>
              <input
                type="text"
                value={editSchema.label}
                onChange={(e) => updateEdit({ label: e.target.value })}
                placeholder="예: 마나"
                className="flex-1 bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
              />
            </div>

            {/* Description */}
            <div className="flex items-start gap-3">
              <label className="w-24 text-sm text-gray-400 pt-2 text-right shrink-0">설명</label>
              <input
                type="text"
                value={editSchema.description}
                onChange={(e) => updateEdit({ description: e.target.value })}
                placeholder="이 속성에 대한 설명"
                className="flex-1 bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
              />
            </div>

            {/* Category */}
            <div className="flex items-start gap-3">
              <label className="w-24 text-sm text-gray-400 pt-2 text-right shrink-0">분류</label>
              <input
                type="text"
                value={editSchema.category}
                onChange={(e) => updateEdit({ category: e.target.value })}
                placeholder="예: 전투, 보상, 행동"
                className="flex-1 bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
              />
            </div>

            {/* Value Type */}
            <div className="flex items-start gap-3">
              <label className="w-24 text-sm text-gray-400 pt-2 text-right shrink-0">값 유형</label>
              <select
                value={editSchema.value_type}
                onChange={(e) => handleValueTypeChange(e.target.value as AttributeValueType)}
                className="flex-1 bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
              >
                {VALUE_TYPES.map((vt) => (
                  <option key={vt} value={vt}>{VALUE_TYPE_LABELS[vt]}</option>
                ))}
              </select>
            </div>

            {/* Default value */}
            <div className="flex items-start gap-3">
              <label className="w-24 text-sm text-gray-400 pt-2 text-right shrink-0">기본값</label>
              <DefaultValueEditor
                valueType={editSchema.value_type as AttributeValueType}
                value={editSchema.default}
                options={editSchema.options}
                onChange={(val) => updateEdit({ default: val })}
              />
            </div>

            {/* Select options (only for select type) */}
            {editSchema.value_type === 'select' && (
              <div className="flex items-start gap-3">
                <label className="w-24 text-sm text-gray-400 pt-2 text-right shrink-0">옵션</label>
                <OptionsEditor
                  options={editSchema.options}
                  onChange={(opts) => updateEdit({ options: opts })}
                />
              </div>
            )}

            {/* Applies to */}
            <div className="flex items-start gap-3">
              <label className="w-24 text-sm text-gray-400 pt-2 text-right shrink-0">적용 대상</label>
              <div className="flex flex-wrap gap-2">
                {collections.map((col) => (
                  <label key={col} className="flex items-center gap-1 text-sm">
                    <input
                      type="checkbox"
                      checked={editSchema.applies_to.includes(col)}
                      onChange={() => toggleAppliesTo(col)}
                    />
                    {col}
                  </label>
                ))}
                {collections.length === 0 && (
                  <span className="text-xs text-gray-500">컬렉션이 없습니다</span>
                )}
              </div>
            </div>
          </div>
        ) : (
          <div className="flex items-center justify-center h-full text-gray-500">
            <p>편집할 속성을 선택하세요</p>
          </div>
        )}
      </div>
    </div>
  );
}

// --- Default Value Editor ---

interface DefaultValueEditorProps {
  valueType: AttributeValueType;
  value: unknown;
  options: SelectOption[];
  onChange: (val: unknown) => void;
}

function DefaultValueEditor({ valueType, value, options, onChange }: DefaultValueEditorProps) {
  switch (valueType) {
    case 'number':
      return (
        <input
          type="number"
          value={typeof value === 'number' ? value : 0}
          onChange={(e) => onChange(Number(e.target.value) || 0)}
          className="flex-1 bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
        />
      );
    case 'string':
      return (
        <input
          type="text"
          value={typeof value === 'string' ? value : ''}
          onChange={(e) => onChange(e.target.value)}
          className="flex-1 bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
        />
      );
    case 'boolean':
      return (
        <input
          type="checkbox"
          checked={typeof value === 'boolean' ? value : false}
          onChange={(e) => onChange(e.target.checked)}
          className="mt-2"
        />
      );
    case 'range': {
      const rangeVal = (value && typeof value === 'object' ? value : { current: 0, max: 0 }) as { current: number; max: number };
      return (
        <div className="flex items-center gap-2 flex-1">
          <label className="text-xs text-gray-500">현재값</label>
          <input
            type="number"
            value={rangeVal.current}
            onChange={(e) => onChange({ ...rangeVal, current: Number(e.target.value) || 0 })}
            className="w-24 bg-gray-700 border border-gray-600 rounded px-2 py-1.5 text-sm"
          />
          <label className="text-xs text-gray-500">최대값</label>
          <input
            type="number"
            value={rangeVal.max}
            onChange={(e) => onChange({ ...rangeVal, max: Number(e.target.value) || 0 })}
            className="w-24 bg-gray-700 border border-gray-600 rounded px-2 py-1.5 text-sm"
          />
        </div>
      );
    }
    case 'select':
      return (
        <select
          value={typeof value === 'string' ? value : ''}
          onChange={(e) => onChange(e.target.value)}
          className="flex-1 bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
        >
          <option value="">-- 선택 --</option>
          {options.map((opt) => (
            <option key={opt.value} value={opt.value}>{opt.label}</option>
          ))}
        </select>
      );
    case 'tags':
      return (
        <div className="flex-1 text-xs text-gray-500 pt-2">
          기본값: 빈 배열 (태그 유형은 배열로 저장됩니다)
        </div>
      );
  }
}

// --- Options Editor (for select type) ---

interface OptionsEditorProps {
  options: SelectOption[];
  onChange: (opts: SelectOption[]) => void;
}

function OptionsEditor({ options, onChange }: OptionsEditorProps) {
  const addOption = () => {
    onChange([...options, { value: '', label: '' }]);
  };

  const updateOption = (index: number, patch: Partial<SelectOption>) => {
    const updated = [...options];
    updated[index] = { ...updated[index], ...patch };
    onChange(updated);
  };

  const removeOption = (index: number) => {
    onChange(options.filter((_, i) => i !== index));
  };

  return (
    <div className="flex-1 space-y-1">
      {options.map((opt, i) => (
        <div key={i} className="flex items-center gap-2">
          <input
            type="text"
            value={opt.value}
            onChange={(e) => updateOption(i, { value: e.target.value })}
            placeholder="값"
            className="w-28 bg-gray-700 border border-gray-600 rounded px-2 py-1 text-xs"
          />
          <input
            type="text"
            value={opt.label}
            onChange={(e) => updateOption(i, { label: e.target.value })}
            placeholder="표시 이름"
            className="flex-1 bg-gray-700 border border-gray-600 rounded px-2 py-1 text-xs"
          />
          <button
            onClick={() => removeOption(i)}
            className="text-gray-500 hover:text-red-400 text-xs"
          >
            x
          </button>
        </div>
      ))}
      <button
        onClick={addOption}
        className="text-xs text-blue-400 hover:text-blue-300"
      >
        + 옵션 추가
      </button>
    </div>
  );
}
