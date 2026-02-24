import { type ReactNode, useCallback, useLayoutEffect, useRef, useState } from 'react';

interface TooltipProps {
  text: string;
  children: ReactNode;
  position?: 'top' | 'bottom';
}

const MARGIN = 8; // minimum distance from viewport edge

export function Tooltip({ text, children, position = 'bottom' }: TooltipProps) {
  const [visible, setVisible] = useState(false);
  const tipRef = useRef<HTMLSpanElement>(null);
  const wrapRef = useRef<HTMLSpanElement>(null);
  const [style, setStyle] = useState<React.CSSProperties>({});
  const [actualPos, setActualPos] = useState(position);

  const reposition = useCallback(() => {
    const tip = tipRef.current;
    const wrap = wrapRef.current;
    if (!tip || !wrap) return;

    const wrapRect = wrap.getBoundingClientRect();
    const tipRect = tip.getBoundingClientRect();
    const vw = window.innerWidth;
    const vh = window.innerHeight;

    // Horizontal: center on parent, then clamp to viewport
    let left = wrapRect.left + wrapRect.width / 2 - tipRect.width / 2;
    if (left < MARGIN) left = MARGIN;
    if (left + tipRect.width > vw - MARGIN) left = vw - MARGIN - tipRect.width;
    // Convert to offset relative to wrapper
    const offsetX = left - wrapRect.left;

    // Vertical: prefer requested position, flip if no room
    let pos = position;
    if (pos === 'bottom' && wrapRect.bottom + tipRect.height + 4 > vh) {
      pos = 'top';
    } else if (pos === 'top' && wrapRect.top - tipRect.height - 4 < 0) {
      pos = 'bottom';
    }
    setActualPos(pos);

    setStyle({
      left: `${offsetX}px`,
      transform: 'none',
    });
  }, [position]);

  useLayoutEffect(() => {
    if (visible) reposition();
  }, [visible, reposition]);

  return (
    <span
      ref={wrapRef}
      className="relative inline-flex"
      onMouseEnter={() => setVisible(true)}
      onMouseLeave={() => setVisible(false)}
    >
      {children}
      {visible && (
        <span
          ref={tipRef}
          style={style}
          className={`absolute z-50 px-2 py-1 text-xs text-gray-200 bg-gray-900 border border-gray-600 rounded shadow-lg whitespace-nowrap pointer-events-none ${
            actualPos === 'top' ? 'bottom-full mb-1' : 'top-full mt-1'
          }`}
        >
          {text}
        </span>
      )}
    </span>
  );
}
