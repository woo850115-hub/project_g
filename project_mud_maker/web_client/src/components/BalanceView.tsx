import { useMemo, useState } from 'react';
import type { ContentItem } from '../types/content';

interface BalanceViewProps {
  collections: Record<string, ContentItem[]>;
  onSelectItem: (collection: string, itemId: string) => void;
}

type SortDir = 'asc' | 'desc';

function numVal(item: ContentItem, key: string): number {
  const v = item[key];
  return typeof v === 'number' ? v : 0;
}

function heatColor(value: number, min: number, max: number): string {
  if (max === min) return '';
  const ratio = (value - min) / (max - min);
  if (ratio < 0.33) return 'bg-green-900/30';
  if (ratio < 0.66) return 'bg-yellow-900/30';
  return 'bg-red-900/30';
}

interface ColumnDef {
  key: string;
  label: string;
  numeric: boolean;
}

const MONSTER_COLS: ColumnDef[] = [
  { key: 'name', label: '이름', numeric: false },
  { key: 'hp', label: 'HP', numeric: true },
  { key: 'attack', label: '공격', numeric: true },
  { key: 'defense', label: '방어', numeric: true },
  { key: 'exp_reward', label: '경험치', numeric: true },
];

const ITEM_COLS: ColumnDef[] = [
  { key: 'name', label: '이름', numeric: false },
  { key: 'item_type', label: '유형', numeric: false },
  { key: 'value', label: '가격', numeric: true },
  { key: 'attack_bonus', label: '공격보너스', numeric: true },
  { key: 'defense_bonus', label: '방어보너스', numeric: true },
  { key: 'heal_amount', label: '회복량', numeric: true },
];

const SKILL_COLS: ColumnDef[] = [
  { key: 'name', label: '이름', numeric: false },
  { key: 'type', label: '유형', numeric: false },
  { key: 'damage_mult', label: '데미지배율', numeric: true },
  { key: 'heal_amount', label: '회복량', numeric: true },
  { key: 'cooldown', label: '쿨다운', numeric: true },
];

function BalanceTable({
  title,
  collection,
  items,
  columns,
  onSelect,
}: {
  title: string;
  collection: string;
  items: ContentItem[];
  columns: ColumnDef[];
  onSelect: (collection: string, id: string) => void;
}) {
  const [sortKey, setSortKey] = useState<string>('name');
  const [sortDir, setSortDir] = useState<SortDir>('asc');

  const handleSort = (key: string) => {
    if (sortKey === key) {
      setSortDir((d) => (d === 'asc' ? 'desc' : 'asc'));
    } else {
      setSortKey(key);
      setSortDir('asc');
    }
  };

  // Compute min/max for heat coloring
  const numericRanges = useMemo(() => {
    const ranges: Record<string, { min: number; max: number }> = {};
    for (const col of columns) {
      if (!col.numeric) continue;
      let min = Infinity;
      let max = -Infinity;
      for (const item of items) {
        const v = numVal(item, col.key);
        if (v < min) min = v;
        if (v > max) max = v;
      }
      ranges[col.key] = { min: min === Infinity ? 0 : min, max: max === -Infinity ? 0 : max };
    }
    return ranges;
  }, [items, columns]);

  const sorted = useMemo(() => {
    const arr = [...items];
    arr.sort((a, b) => {
      const col = columns.find((c) => c.key === sortKey);
      if (col?.numeric) {
        const diff = numVal(a, sortKey) - numVal(b, sortKey);
        return sortDir === 'asc' ? diff : -diff;
      }
      const av = String(a[sortKey] ?? '');
      const bv = String(b[sortKey] ?? '');
      return sortDir === 'asc' ? av.localeCompare(bv) : bv.localeCompare(av);
    });
    return arr;
  }, [items, sortKey, sortDir, columns]);

  if (items.length === 0) return null;

  return (
    <div className="mb-6">
      <h3 className="text-sm font-bold text-gray-300 mb-2">{title} ({items.length})</h3>
      <div className="overflow-x-auto">
        <table className="w-full text-xs border-collapse">
          <thead>
            <tr className="border-b border-gray-700">
              {columns.map((col) => (
                <th
                  key={col.key}
                  onClick={() => handleSort(col.key)}
                  className="px-3 py-2 text-left text-gray-400 cursor-pointer hover:text-gray-200 select-none"
                >
                  {col.label}
                  {sortKey === col.key && (sortDir === 'asc' ? ' ^' : ' v')}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {sorted.map((item) => (
              <tr
                key={item.id}
                onClick={() => onSelect(collection, item.id)}
                className="border-b border-gray-800 hover:bg-gray-700/30 cursor-pointer"
              >
                {columns.map((col) => {
                  const val = col.numeric ? numVal(item, col.key) : item[col.key];
                  const range = col.numeric ? numericRanges[col.key] : undefined;
                  const heat = range ? heatColor(val as number, range.min, range.max) : '';
                  return (
                    <td key={col.key} className={`px-3 py-1.5 ${heat}`}>
                      {val !== undefined && val !== null ? String(val) : '-'}
                    </td>
                  );
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

export function BalanceView({ collections, onSelectItem }: BalanceViewProps) {
  const monsters = collections['monsters'] || [];
  const items = collections['items'] || [];
  const skills = collections['skills'] || [];

  if (monsters.length === 0 && items.length === 0 && skills.length === 0) {
    return (
      <div className="flex items-center justify-center h-full text-gray-500 text-sm">
        밸런스 데이터가 없습니다. 먼저 monsters, items, skills 컬렉션을 추가하세요.
      </div>
    );
  }

  return (
    <div className="p-6 overflow-y-auto h-full">
      <BalanceTable
        title="몬스터"
        collection="monsters"
        items={monsters}
        columns={MONSTER_COLS}
        onSelect={onSelectItem}
      />
      <BalanceTable
        title="아이템"
        collection="items"
        items={items}
        columns={ITEM_COLS}
        onSelect={onSelectItem}
      />
      <BalanceTable
        title="스킬"
        collection="skills"
        items={skills}
        columns={SKILL_COLS}
        onSelect={onSelectItem}
      />
    </div>
  );
}
