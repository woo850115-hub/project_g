import { useCallback, useEffect, useRef, useState } from 'react';

const MAX_HISTORY = 50;

export interface HistoryControls<T> {
  state: T;
  set: (newState: T) => void;
  undo: () => void;
  redo: () => void;
  canUndo: boolean;
  canRedo: boolean;
  /** Replace current state without pushing to history (e.g. for continuous edits like dragging) */
  replace: (newState: T) => void;
}

export function useHistory<T>(initial: T): HistoryControls<T> {
  const [present, setPresent] = useState<T>(initial);
  const pastRef = useRef<T[]>([]);
  const futureRef = useRef<T[]>([]);
  const [, forceUpdate] = useState(0);

  const set = useCallback((newState: T) => {
    setPresent((prev) => {
      pastRef.current = [...pastRef.current, prev].slice(-MAX_HISTORY);
      futureRef.current = [];
      forceUpdate((n) => n + 1);
      return newState;
    });
  }, []);

  const replace = useCallback((newState: T) => {
    setPresent(newState);
  }, []);

  const undo = useCallback(() => {
    if (pastRef.current.length === 0) return;
    setPresent((prev) => {
      const past = [...pastRef.current];
      const previous = past.pop()!;
      pastRef.current = past;
      futureRef.current = [prev, ...futureRef.current].slice(0, MAX_HISTORY);
      forceUpdate((n) => n + 1);
      return previous;
    });
  }, []);

  const redo = useCallback(() => {
    if (futureRef.current.length === 0) return;
    setPresent((prev) => {
      const future = [...futureRef.current];
      const next = future.shift()!;
      futureRef.current = future;
      pastRef.current = [...pastRef.current, prev].slice(-MAX_HISTORY);
      forceUpdate((n) => n + 1);
      return next;
    });
  }, []);

  // Global keyboard shortcuts
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === 'z' && !e.shiftKey) {
        e.preventDefault();
        undo();
      }
      if ((e.ctrlKey || e.metaKey) && (e.key === 'y' || (e.key === 'z' && e.shiftKey))) {
        e.preventDefault();
        redo();
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [undo, redo]);

  return {
    state: present,
    set,
    undo,
    redo,
    canUndo: pastRef.current.length > 0,
    canRedo: futureRef.current.length > 0,
    replace,
  };
}
