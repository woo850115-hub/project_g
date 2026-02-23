import { useEffect, useRef, useState, type ReactNode } from 'react';

const DIR_LABELS: Record<string, string> = {
  north: '\uBD81\uCABD', south: '\uB0A8\uCABD',
  east: '\uB3D9\uCABD', west: '\uC11C\uCABD',
  up: '\uC704', down: '\uC544\uB798',
};

interface ModalProps {
  open: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
  width?: string;
}

export function Modal({ open, onClose, title, children, width = 'max-w-md' }: ModalProps) {
  const overlayRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div
      ref={overlayRef}
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60"
      onMouseDown={(e) => {
        if (e.target === overlayRef.current) onClose();
      }}
    >
      <div className={`${width} w-full bg-gray-800 border border-gray-600 rounded-lg shadow-2xl`}>
        <div className="flex items-center justify-between px-4 py-3 border-b border-gray-700">
          <h3 className="text-sm font-semibold text-gray-100">{title}</h3>
          <button
            onClick={onClose}
            className="text-gray-500 hover:text-gray-300 text-lg leading-none"
          >
            &times;
          </button>
        </div>
        <div className="p-4">{children}</div>
      </div>
    </div>
  );
}

// --- Confirm Dialog ---

interface ConfirmDialogProps {
  open: boolean;
  title: string;
  message: string;
  confirmLabel?: string;
  confirmClass?: string;
  onConfirm: () => void;
  onCancel: () => void;
}

export function ConfirmDialog({
  open,
  title,
  message,
  confirmLabel = '\uD655\uC778',
  confirmClass = 'bg-red-600 hover:bg-red-500',
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  return (
    <Modal open={open} onClose={onCancel} title={title}>
      <p className="text-sm text-gray-300 mb-4">{message}</p>
      <div className="flex justify-end gap-2">
        <button
          onClick={onCancel}
          className="px-3 py-1.5 text-sm bg-gray-700 hover:bg-gray-600 rounded"
        >
          취소
        </button>
        <button
          onClick={onConfirm}
          className={`px-3 py-1.5 text-sm rounded text-white ${confirmClass}`}
        >
          {confirmLabel}
        </button>
      </div>
    </Modal>
  );
}

// --- Prompt Dialog ---

interface PromptDialogProps {
  open: boolean;
  title: string;
  label: string;
  placeholder?: string;
  defaultValue?: string;
  onSubmit: (value: string) => void;
  onCancel: () => void;
}

export function PromptDialog({
  open,
  title,
  label,
  placeholder = '',
  defaultValue = '',
  onSubmit,
  onCancel,
}: PromptDialogProps) {
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) {
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [open]);

  const handleSubmit = () => {
    const val = inputRef.current?.value.trim();
    if (val) onSubmit(val);
  };

  return (
    <Modal open={open} onClose={onCancel} title={title}>
      <label className="block text-xs text-gray-400 mb-1">{label}</label>
      <input
        ref={inputRef}
        type="text"
        defaultValue={defaultValue}
        placeholder={placeholder}
        onKeyDown={(e) => {
          if (e.key === 'Enter') handleSubmit();
        }}
        className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm mb-4"
      />
      <div className="flex justify-end gap-2">
        <button
          onClick={onCancel}
          className="px-3 py-1.5 text-sm bg-gray-700 hover:bg-gray-600 rounded"
        >
          취소
        </button>
        <button
          onClick={handleSubmit}
          className="px-3 py-1.5 text-sm bg-blue-600 hover:bg-blue-500 rounded text-white"
        >
          확인
        </button>
      </div>
    </Modal>
  );
}

// --- Select Dialog ---

interface SelectOption {
  value: string;
  label: string;
}

interface SelectDialogProps {
  open: boolean;
  title: string;
  label: string;
  options: SelectOption[];
  onSelect: (value: string) => void;
  onCancel: () => void;
}

export function SelectDialog({
  open,
  title,
  label,
  options,
  onSelect,
  onCancel,
}: SelectDialogProps) {
  const selectRef = useRef<HTMLSelectElement>(null);

  useEffect(() => {
    if (open) {
      setTimeout(() => selectRef.current?.focus(), 50);
    }
  }, [open]);

  const handleSubmit = () => {
    const val = selectRef.current?.value;
    if (val) onSelect(val);
  };

  return (
    <Modal open={open} onClose={onCancel} title={title}>
      <label className="block text-xs text-gray-400 mb-1">{label}</label>
      <select
        ref={selectRef}
        defaultValue={options[0]?.value}
        onKeyDown={(e) => {
          if (e.key === 'Enter') handleSubmit();
        }}
        className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm mb-4"
      >
        {options.map((opt) => (
          <option key={opt.value} value={opt.value}>
            {opt.label}
          </option>
        ))}
      </select>
      <div className="flex justify-end gap-2">
        <button
          onClick={onCancel}
          className="px-3 py-1.5 text-sm bg-gray-700 hover:bg-gray-600 rounded"
        >
          취소
        </button>
        <button
          onClick={handleSubmit}
          className="px-3 py-1.5 text-sm bg-blue-600 hover:bg-blue-500 rounded text-white"
        >
          확인
        </button>
      </div>
    </Modal>
  );
}

// --- Multi-step Room Exit Dialog ---

interface AddExitDialogProps {
  open: boolean;
  availableDirections: string[];
  targetRooms: { id: string; name: string }[];
  onSubmit: (direction: string, targetId: string) => void;
  onCancel: () => void;
}

export function AddExitDialog({
  open,
  availableDirections,
  targetRooms,
  onSubmit,
  onCancel,
}: AddExitDialogProps) {
  const dirRef = useRef<HTMLSelectElement>(null);
  const targetRef = useRef<HTMLSelectElement>(null);

  const handleSubmit = () => {
    const dir = dirRef.current?.value;
    const target = targetRef.current?.value;
    if (dir && target) onSubmit(dir, target);
  };

  if (availableDirections.length === 0 || targetRooms.length === 0) {
    return (
      <Modal open={open} onClose={onCancel} title="출구 추가">
        <p className="text-sm text-gray-400 mb-4">
          {availableDirections.length === 0
            ? '사용 가능한 방향이 없습니다.'
            : '연결할 다른 방이 없습니다.'}
        </p>
        <div className="flex justify-end">
          <button
            onClick={onCancel}
            className="px-3 py-1.5 text-sm bg-gray-700 hover:bg-gray-600 rounded"
          >
            닫기
          </button>
        </div>
      </Modal>
    );
  }

  return (
    <Modal open={open} onClose={onCancel} title="출구 추가">
      <div className="space-y-3 mb-4">
        <div>
          <label className="block text-xs text-gray-400 mb-1">방향</label>
          <select
            ref={dirRef}
            defaultValue={availableDirections[0]}
            className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
          >
            {availableDirections.map((d) => (
              <option key={d} value={d}>
                {DIR_LABELS[d] || d}
              </option>
            ))}
          </select>
        </div>
        <div>
          <label className="block text-xs text-gray-400 mb-1">대상 방</label>
          <select
            ref={targetRef}
            defaultValue={targetRooms[0]?.id}
            className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
          >
            {targetRooms.map((r) => (
              <option key={r.id} value={r.id}>
                {r.name} ({r.id})
              </option>
            ))}
          </select>
        </div>
      </div>
      <div className="flex justify-end gap-2">
        <button
          onClick={onCancel}
          className="px-3 py-1.5 text-sm bg-gray-700 hover:bg-gray-600 rounded"
        >
          취소
        </button>
        <button
          onClick={handleSubmit}
          className="px-3 py-1.5 text-sm bg-blue-600 hover:bg-blue-500 rounded text-white"
        >
          출구 추가
        </button>
      </div>
    </Modal>
  );
}

// --- Add Entity Dialog ---

interface AddEntityDialogProps {
  open: boolean;
  contentItems: Record<string, { id: string; name?: string }[]>;
  onSubmit: (type: string, contentId: string) => void;
  onCancel: () => void;
}

export function AddEntityDialog({
  open,
  contentItems,
  onSubmit,
  onCancel,
}: AddEntityDialogProps) {
  const [selectedType, setSelectedType] = useState('npc');
  const contentSelectRef = useRef<HTMLSelectElement>(null);
  const contentInputRef = useRef<HTMLInputElement>(null);

  const entityTypes = [
    { value: 'npc', label: 'NPC', collection: 'monsters' },
    { value: 'item', label: '\uC544\uC774\uD15C', collection: 'items' },
  ];

  const collection = entityTypes.find((t) => t.value === selectedType)?.collection || 'monsters';
  const items = contentItems[collection] || [];

  const handleSubmit = () => {
    const contentId = items.length > 0
      ? contentSelectRef.current?.value
      : contentInputRef.current?.value?.trim();
    if (selectedType && contentId) onSubmit(selectedType, contentId);
  };

  return (
    <Modal open={open} onClose={onCancel} title="엔티티 추가">
      <div className="space-y-3 mb-4">
        <div>
          <label className="block text-xs text-gray-400 mb-1">엔티티 유형</label>
          <select
            value={selectedType}
            onChange={(e) => setSelectedType(e.target.value)}
            className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
          >
            {entityTypes.map((t) => (
              <option key={t.value} value={t.value}>
                {t.label}
              </option>
            ))}
          </select>
        </div>
        <div>
          <label className="block text-xs text-gray-400 mb-1">콘텐츠</label>
          {items.length > 0 ? (
            <select
              ref={contentSelectRef}
              defaultValue={items[0]?.id}
              className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
            >
              {items.map((item) => (
                <option key={item.id} value={item.id}>
                  {item.name || item.id}
                </option>
              ))}
            </select>
          ) : (
            <input
              ref={contentInputRef}
              type="text"
              placeholder="콘텐츠 ID"
              className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
            />
          )}
        </div>
      </div>
      <div className="flex justify-end gap-2">
        <button
          onClick={onCancel}
          className="px-3 py-1.5 text-sm bg-gray-700 hover:bg-gray-600 rounded"
        >
          취소
        </button>
        <button
          onClick={handleSubmit}
          className="px-3 py-1.5 text-sm bg-blue-600 hover:bg-blue-500 rounded text-white"
        >
          엔티티 추가
        </button>
      </div>
    </Modal>
  );
}

// --- Add Room Dialog ---

interface AddRoomDialogProps {
  open: boolean;
  existingIds: string[];
  onSubmit: (id: string, name: string) => void;
  onCancel: () => void;
}

export function AddRoomDialog({
  open,
  existingIds,
  onSubmit,
  onCancel,
}: AddRoomDialogProps) {
  const idRef = useRef<HTMLInputElement>(null);
  const nameRef = useRef<HTMLInputElement>(null);
  const errorRef = useRef<HTMLParagraphElement>(null);

  useEffect(() => {
    if (open) setTimeout(() => idRef.current?.focus(), 50);
  }, [open]);

  const handleSubmit = () => {
    const id = idRef.current?.value.trim();
    const name = nameRef.current?.value.trim() || id;
    if (!id) return;
    if (existingIds.includes(id)) {
      if (errorRef.current) errorRef.current.textContent = `방 ID "${id}"이(가) 이미 존재합니다`;
      return;
    }
    onSubmit(id, name!);
  };

  return (
    <Modal open={open} onClose={onCancel} title="방 추가">
      <div className="space-y-3 mb-4">
        <div>
          <label className="block text-xs text-gray-400 mb-1">방 ID</label>
          <input
            ref={idRef}
            type="text"
            placeholder="예: tavern"
            onKeyDown={(e) => {
              if (e.key === 'Enter') nameRef.current?.focus();
            }}
            className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
          />
        </div>
        <div>
          <label className="block text-xs text-gray-400 mb-1">방 이름</label>
          <input
            ref={nameRef}
            type="text"
            placeholder="예: 오래된 선술집"
            onKeyDown={(e) => {
              if (e.key === 'Enter') handleSubmit();
            }}
            className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
          />
        </div>
        <p ref={errorRef} className="text-xs text-red-400"></p>
      </div>
      <div className="flex justify-end gap-2">
        <button
          onClick={onCancel}
          className="px-3 py-1.5 text-sm bg-gray-700 hover:bg-gray-600 rounded"
        >
          취소
        </button>
        <button
          onClick={handleSubmit}
          className="px-3 py-1.5 text-sm bg-blue-600 hover:bg-blue-500 rounded text-white"
        >
          방 추가
        </button>
      </div>
    </Modal>
  );
}

// --- Connect Rooms Dialog ---

interface ConnectDialogProps {
  open: boolean;
  sourceId: string;
  targetId: string;
  directions: string[];
  onSubmit: (direction: string, bidirectional: boolean) => void;
  onCancel: () => void;
}

export function ConnectDialog({
  open,
  sourceId,
  targetId,
  directions,
  onSubmit,
  onCancel,
}: ConnectDialogProps) {
  const dirRef = useRef<HTMLSelectElement>(null);
  const bidirRef = useRef<HTMLInputElement>(null);

  const handleSubmit = () => {
    const dir = dirRef.current?.value;
    const bidir = bidirRef.current?.checked ?? false;
    if (dir) onSubmit(dir, bidir);
  };

  return (
    <Modal open={open} onClose={onCancel} title="방 연결">
      <p className="text-xs text-gray-400 mb-3">
        <span className="text-blue-400">{sourceId}</span>
        {' \u2192 '}
        <span className="text-green-400">{targetId}</span>
      </p>
      <div className="space-y-3 mb-4">
        <div>
          <label className="block text-xs text-gray-400 mb-1">방향</label>
          <select
            ref={dirRef}
            defaultValue={directions[0]}
            className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm"
          >
            {directions.map((d) => (
              <option key={d} value={d}>
                {DIR_LABELS[d] || d}
              </option>
            ))}
          </select>
        </div>
        <label className="flex items-center gap-2 text-sm text-gray-300">
          <input ref={bidirRef} type="checkbox" defaultChecked className="rounded" />
          양방향 출구
        </label>
      </div>
      <div className="flex justify-end gap-2">
        <button
          onClick={onCancel}
          className="px-3 py-1.5 text-sm bg-gray-700 hover:bg-gray-600 rounded"
        >
          취소
        </button>
        <button
          onClick={handleSubmit}
          className="px-3 py-1.5 text-sm bg-blue-600 hover:bg-blue-500 rounded text-white"
        >
          연결
        </button>
      </div>
    </Modal>
  );
}
