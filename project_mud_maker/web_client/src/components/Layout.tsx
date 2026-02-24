import { type ReactNode, useState } from 'react';
import { generateAllApi, serverApi } from '../api/client';

export type TabId = 'map' | 'database' | 'scripts' | 'preview';

interface LayoutProps {
  activeTab: TabId;
  onTabChange: (tab: TabId) => void;
  children: ReactNode;
}

const tabs: { id: TabId; label: string }[] = [
  { id: 'map', label: '맵' },
  { id: 'database', label: '데이터베이스' },
  { id: 'scripts', label: '스크립트' },
  { id: 'preview', label: '미리보기' },
];

export function Layout({ activeTab, onTabChange, children }: LayoutProps) {
  const [generating, setGenerating] = useState(false);

  const handleGenerateAndRestart = async () => {
    setGenerating(true);
    try {
      await generateAllApi.generateAll();
      await serverApi.restart();
    } catch {
      // errors shown in individual pages
    } finally {
      setGenerating(false);
    }
  };

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
        <div className="ml-auto">
          <button
            onClick={handleGenerateAndRestart}
            disabled={generating}
            className="px-3 py-1.5 text-xs font-medium bg-green-600 hover:bg-green-500 disabled:opacity-50 rounded flex items-center gap-1.5"
            title="모든 Lua 생성 + 서버 재시작"
          >
            {generating ? '생성 중...' : '전체 생성 + 재시작'}
          </button>
        </div>
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
