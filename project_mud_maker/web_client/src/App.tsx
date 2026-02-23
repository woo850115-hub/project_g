import { useState } from 'react';
import { Layout, type TabId } from './components/Layout';
import { MapEditor } from './pages/MapEditor';
import { Database } from './pages/Database';
import { TriggerEditor } from './pages/TriggerEditor';
import { ScriptEditor } from './pages/ScriptEditor';
import { Preview } from './pages/Preview';

function App() {
  const [activeTab, setActiveTab] = useState<TabId>('database');

  return (
    <Layout activeTab={activeTab} onTabChange={setActiveTab}>
      {activeTab === 'map' && <MapEditor />}
      {activeTab === 'database' && <Database />}
      {activeTab === 'triggers' && <TriggerEditor />}
      {activeTab === 'scripts' && <ScriptEditor />}
      {activeTab === 'preview' && <Preview />}
    </Layout>
  );
}

export default App;
