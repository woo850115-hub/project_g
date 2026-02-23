import { type ReactNode } from 'react';

export type TabId = 'map' | 'database' | 'triggers' | 'scripts' | 'preview';

interface LayoutProps {
  activeTab: TabId;
  onTabChange: (tab: TabId) => void;
  children: ReactNode;
}

const tabs: { id: TabId; label: string }[] = [
  { id: 'map', label: '\uB9F5' },
  { id: 'database', label: '\uB370\uC774\uD130\uBCA0\uC774\uC2A4' },
  { id: 'triggers', label: '\uD2B8\uB9AC\uAC70' },
  { id: 'scripts', label: '\uC2A4\uD06C\uB9BD\uD2B8' },
  { id: 'preview', label: '\uBBF8\uB9AC\uBCF4\uAE30' },
];

export function Layout({ activeTab, onTabChange, children }: LayoutProps) {
  return (
    <div className="flex flex-col h-screen bg-gray-900 text-gray-100">
      {/* Header */}
      <header className="flex items-center border-b border-gray-700 bg-gray-800 px-4">
        <h1 className="text-lg font-bold mr-8 py-3">MUD 게임 메이커</h1>
        <nav className="flex gap-1">
          {tabs.map((tab) => (
            <button
              key={tab.id}
              onClick={() => onTabChange(tab.id)}
              className={`px-4 py-3 text-sm font-medium transition-colors ${
                activeTab === tab.id
                  ? 'text-blue-400 border-b-2 border-blue-400'
                  : 'text-gray-400 hover:text-gray-200'
              }`}
            >
              {tab.label}
            </button>
          ))}
        </nav>
      </header>

      {/* Content */}
      <main className="flex-1 overflow-hidden">{children}</main>

      {/* Status bar */}
      <footer className="flex items-center gap-4 px-4 py-1.5 text-xs text-gray-500 border-t border-gray-700 bg-gray-800">
        <span>MUD 게임 메이커 v0.1</span>
      </footer>
    </div>
  );
}
