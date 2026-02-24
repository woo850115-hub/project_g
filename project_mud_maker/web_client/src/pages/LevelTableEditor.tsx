import { useCallback, useEffect, useState } from 'react';
import { levelTableApi } from '../api/client';
import type { LevelEntry } from '../types/level_table';

const EMPTY_ENTRY: Omit<LevelEntry, 'level'> = {
  exp_required: 0,
  hp_bonus: 0,
  mp_bonus: 0,
  atk_bonus: 0,
  def_bonus: 0,
};

export function LevelTableEditor() {
  const [entries, setEntries] = useState<LevelEntry[]>([]);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);

  const load = useCallback(async () => {
    try {
      const data = await levelTableApi.list();
      setEntries(data);
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to load');
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const updateEntry = (index: number, field: keyof Omit<LevelEntry, 'level'>, value: number) => {
    const next = [...entries];
    next[index] = { ...next[index], [field]: value };
    setEntries(next);
    setSaved(false);
  };

  const addRow = () => {
    const nextLevel = entries.length > 0 ? entries[entries.length - 1].level + 1 : 1;
    setEntries([...entries, { level: nextLevel, ...EMPTY_ENTRY }]);
    setSaved(false);
  };

  const removeRow = (index: number) => {
    const next = entries.filter((_, i) => i !== index);
    // Re-number levels sequentially
    const renumbered = next.map((e, i) => ({ ...e, level: i + 1 }));
    setEntries(renumbered);
    setSaved(false);
  };

  const save = async () => {
    setSaving(true);
    setError(null);
    try {
      await levelTableApi.save(entries);
      setSaved(true);
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Save failed');
    } finally {
      setSaving(false);
    }
  };

  const fields: { key: keyof Omit<LevelEntry, 'level'>; label: string }[] = [
    { key: 'exp_required', label: '필요 경험치' },
    { key: 'hp_bonus', label: 'HP 보너스' },
    { key: 'mp_bonus', label: 'MP 보너스' },
    { key: 'atk_bonus', label: '공격력' },
    { key: 'def_bonus', label: '방어력' },
  ];

  return (
    <div className="p-6 max-w-4xl">
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-lg font-bold">레벨 테이블</h2>
        <div className="flex items-center gap-2">
          {saved && (
            <span className="text-xs text-green-400">저장됨</span>
          )}
          <button
            onClick={save}
            disabled={saving}
            className="px-4 py-1.5 text-sm bg-blue-600 hover:bg-blue-500 disabled:opacity-50 rounded"
          >
            {saving ? '저장 중...' : '저장'}
          </button>
        </div>
      </div>

      {error && (
        <div className="mb-4 bg-red-600/20 border border-red-600 rounded px-3 py-2 text-sm text-red-400">
          {error}
          <button className="ml-2 font-bold" onClick={() => setError(null)}>x</button>
        </div>
      )}

      <div className="overflow-x-auto">
        <table className="w-full text-sm border-collapse">
          <thead>
            <tr className="border-b border-gray-700">
              <th className="text-left px-2 py-2 text-xs text-gray-400 w-16">레벨</th>
              {fields.map((f) => (
                <th key={f.key} className="text-left px-2 py-2 text-xs text-gray-400">
                  {f.label}
                </th>
              ))}
              <th className="w-10" />
            </tr>
          </thead>
          <tbody>
            {entries.map((entry, i) => (
              <tr key={i} className="border-b border-gray-700/50 hover:bg-gray-800/50">
                <td className="px-2 py-1.5 text-gray-400 font-mono">{entry.level}</td>
                {fields.map((f) => (
                  <td key={f.key} className="px-1 py-1">
                    <input
                      type="number"
                      value={entry[f.key]}
                      onChange={(e) =>
                        updateEntry(i, f.key, e.target.value === '' ? 0 : Number(e.target.value))
                      }
                      className="w-full bg-gray-700 border border-gray-600 rounded px-2 py-1 text-sm text-center"
                    />
                  </td>
                ))}
                <td className="px-1 py-1 text-center">
                  <button
                    onClick={() => removeRow(i)}
                    className="text-gray-500 hover:text-red-400 text-xs"
                    title="행 삭제"
                  >
                    x
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <button
        onClick={addRow}
        className="mt-3 text-sm text-blue-400 hover:text-blue-300"
      >
        + 행 추가
      </button>

      {entries.length === 0 && (
        <p className="mt-4 text-sm text-gray-500 text-center">
          레벨 테이블이 비어있습니다. 행을 추가하세요.
        </p>
      )}
    </div>
  );
}
