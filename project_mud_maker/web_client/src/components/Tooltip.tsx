import { type ReactNode, useState } from 'react';

interface TooltipProps {
  text: string;
  children: ReactNode;
  position?: 'top' | 'bottom';
}

export function Tooltip({ text, children, position = 'bottom' }: TooltipProps) {
  const [visible, setVisible] = useState(false);

  return (
    <span
      className="relative inline-flex"
      onMouseEnter={() => setVisible(true)}
      onMouseLeave={() => setVisible(false)}
    >
      {children}
      {visible && (
        <span
          className={`absolute left-1/2 -translate-x-1/2 z-50 px-2 py-1 text-xs text-gray-200 bg-gray-900 border border-gray-600 rounded shadow-lg whitespace-nowrap pointer-events-none ${
            position === 'top' ? 'bottom-full mb-1' : 'top-full mt-1'
          }`}
        >
          {text}
        </span>
      )}
    </span>
  );
}
