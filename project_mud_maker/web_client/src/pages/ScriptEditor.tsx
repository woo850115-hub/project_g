import { useCallback, useEffect, useRef, useState } from 'react';
import Editor, { type OnMount, type Monaco } from '@monaco-editor/react';
import type { editor } from 'monaco-editor';
import { scriptsApi } from '../api/client';
import type { ScriptFile } from '../types/api';

// Lua API completions for the MUD engine
const LUA_API_ITEMS = [
  // ecs
  { label: 'ecs:get', insert: 'ecs:get(${1:entity}, "${2:component}")', doc: 'Get component value' },
  { label: 'ecs:set', insert: 'ecs:set(${1:entity}, "${2:component}", ${3:value})', doc: 'Set component value' },
  { label: 'ecs:has', insert: 'ecs:has(${1:entity}, "${2:component}")', doc: 'Check if entity has component' },
  { label: 'ecs:remove', insert: 'ecs:remove(${1:entity}, "${2:component}")', doc: 'Remove component' },
  { label: 'ecs:spawn', insert: 'ecs:spawn()', doc: 'Spawn new entity' },
  { label: 'ecs:despawn', insert: 'ecs:despawn(${1:entity})', doc: 'Despawn entity' },
  { label: 'ecs:query', insert: 'ecs:query(${1:components})', doc: 'Query entities with components' },
  // space
  { label: 'space:entity_room', insert: 'space:entity_room(${1:entity})', doc: 'Get entity room' },
  { label: 'space:move_entity', insert: 'space:move_entity(${1:entity}, "${2:direction}")', doc: 'Move entity' },
  { label: 'space:place_entity', insert: 'space:place_entity(${1:entity}, ${2:room})', doc: 'Place entity in room' },
  { label: 'space:remove_entity', insert: 'space:remove_entity(${1:entity})', doc: 'Remove entity from space' },
  { label: 'space:room_occupants', insert: 'space:room_occupants(${1:room})', doc: 'Get entities in room' },
  { label: 'space:register_room', insert: 'space:register_room(${1:room}, ${2:exits})', doc: 'Register room with exits' },
  { label: 'space:exits', insert: 'space:exits(${1:room})', doc: 'Get room exits' },
  { label: 'space:room_exists', insert: 'space:room_exists(${1:room})', doc: 'Check if room exists' },
  { label: 'space:room_count', insert: 'space:room_count()', doc: 'Get total room count' },
  { label: 'space:all_rooms', insert: 'space:all_rooms()', doc: 'Get all room entities' },
  // output
  { label: 'output:send', insert: 'output:send(${1:session_id}, "${2:text}")', doc: 'Send text to session' },
  { label: 'output:broadcast_room', insert: 'output:broadcast_room(${1:room}, "${2:text}")', doc: 'Broadcast to room' },
  // sessions
  { label: 'sessions:session_for', insert: 'sessions:session_for(${1:entity})', doc: 'Get session for entity' },
  { label: 'sessions:playing_list', insert: 'sessions:playing_list()', doc: 'Get all playing sessions' },
  // hooks
  { label: 'hooks.on_init', insert: 'hooks.on_init(function()\n\t${1}\nend)', doc: 'Register init hook' },
  { label: 'hooks.on_tick', insert: 'hooks.on_tick(function()\n\t${1}\nend)', doc: 'Register tick hook' },
  { label: 'hooks.on_action', insert: 'hooks.on_action("${1:command}", function(ctx)\n\t${2}\n\treturn true\nend)', doc: 'Register action hook' },
  { label: 'hooks.on_enter_room', insert: 'hooks.on_enter_room(function(entity, room)\n\t${1}\nend)', doc: 'Register enter room hook' },
  { label: 'hooks.on_connect', insert: 'hooks.on_connect(function(session_id, entity)\n\t${1}\nend)', doc: 'Register connect hook' },
  { label: 'hooks.on_admin', insert: 'hooks.on_admin("${1:command}", ${2:0}, function(ctx)\n\t${3}\nend)', doc: 'Register admin hook' },
  // log
  { label: 'log.info', insert: 'log.info("${1:message}")', doc: 'Log info message' },
  { label: 'log.warn', insert: 'log.warn("${1:message}")', doc: 'Log warning' },
  { label: 'log.error', insert: 'log.error("${1:message}")', doc: 'Log error' },
  { label: 'log.debug', insert: 'log.debug("${1:message}")', doc: 'Log debug message' },
];

export function ScriptEditor() {
  const [files, setFiles] = useState<ScriptFile[]>([]);
  const [activeFile, setActiveFile] = useState<string | null>(null);
  const [content, setContent] = useState('');
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const editorRef = useRef<Parameters<OnMount>[0] | null>(null);

  const loadFiles = useCallback(async () => {
    try {
      const list = await scriptsApi.list();
      setFiles(list);
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to load scripts');
    }
  }, []);

  useEffect(() => {
    loadFiles();
  }, [loadFiles]);

  const openFile = async (filename: string) => {
    if (dirty && !confirm('Unsaved changes will be lost. Continue?')) return;
    try {
      const data = await scriptsApi.get(filename);
      setActiveFile(filename);
      setContent(data.content);
      setDirty(false);
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to open file');
    }
  };

  const saveFile = async () => {
    if (!activeFile) return;
    setSaving(true);
    try {
      await scriptsApi.update(activeFile, content);
      setDirty(false);
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Save failed');
    } finally {
      setSaving(false);
    }
  };

  const createFile = async () => {
    const filename = prompt('Enter filename (e.g. 05_quests.lua):');
    if (!filename) return;
    if (!filename.endsWith('.lua')) {
      setError('Filename must end with .lua');
      return;
    }
    try {
      await scriptsApi.create(filename, `-- ${filename}\n`);
      await loadFiles();
      await openFile(filename);
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Create failed');
    }
  };

  const deleteFile = async () => {
    if (!activeFile) return;
    if (!confirm(`Delete "${activeFile}"?`)) return;
    try {
      await scriptsApi.delete(activeFile);
      setActiveFile(null);
      setContent('');
      setDirty(false);
      await loadFiles();
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Delete failed');
    }
  };

  const handleEditorMount: OnMount = (editorInstance: editor.IStandaloneCodeEditor, monaco: Monaco) => {
    editorRef.current = editorInstance;

    // Ctrl+S save
    editorInstance.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => {
      saveFile();
    });

    // Register Lua completion provider
    monaco.languages.registerCompletionItemProvider('lua', {
      provideCompletionItems: (model: editor.ITextModel, position: { lineNumber: number; column: number }) => {
        const word = model.getWordUntilPosition(position);
        const range = {
          startLineNumber: position.lineNumber,
          endLineNumber: position.lineNumber,
          startColumn: word.startColumn,
          endColumn: word.endColumn,
        };

        // Also check the text before the cursor for `:` or `.` triggers
        const lineContent = model.getLineContent(position.lineNumber);
        const textBefore = lineContent.substring(0, position.column - 1);

        const suggestions = LUA_API_ITEMS.filter((item) => {
          // Match if typing the prefix (e.g. "ecs", "space", "hooks")
          return (
            item.label.toLowerCase().includes(word.word.toLowerCase()) ||
            textBefore.endsWith(item.label.split(/[:.]/)[0] + ':') ||
            textBefore.endsWith(item.label.split(/[:.]/)[0] + '.')
          );
        }).map((item) => ({
          label: item.label,
          kind: monaco.languages.CompletionItemKind.Function,
          insertText: item.insert,
          insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
          documentation: item.doc,
          range,
        }));

        return { suggestions };
      },
      triggerCharacters: [':', '.'],
    });
  };

  return (
    <div className="flex h-full">
      {/* Error toast */}
      {error && (
        <div className="fixed top-4 right-4 bg-red-600 text-white px-4 py-2 rounded shadow-lg z-50">
          {error}
          <button className="ml-2 font-bold" onClick={() => setError(null)}>x</button>
        </div>
      )}

      {/* File list sidebar */}
      <div className="w-56 border-r border-gray-700 bg-gray-800 flex flex-col">
        <div className="p-3 border-b border-gray-700 flex items-center justify-between">
          <span className="text-sm font-medium text-gray-300">Scripts</span>
          <button
            onClick={createFile}
            className="text-xs px-2 py-1 bg-blue-600 hover:bg-blue-500 rounded"
          >
            + New
          </button>
        </div>
        <div className="flex-1 overflow-y-auto">
          {files.map((f) => (
            <button
              key={f.filename}
              onClick={() => openFile(f.filename)}
              className={`w-full text-left px-3 py-2 text-sm border-b border-gray-700/50 transition-colors ${
                activeFile === f.filename
                  ? 'bg-blue-900/40 text-blue-300'
                  : 'text-gray-400 hover:bg-gray-700/50 hover:text-gray-200'
              }`}
            >
              {f.filename}
              <span className="text-xs text-gray-600 ml-1">
                ({Math.round(f.size / 1024 * 10) / 10}k)
              </span>
            </button>
          ))}
        </div>
      </div>

      {/* Editor area */}
      <div className="flex-1 flex flex-col">
        {activeFile ? (
          <>
            {/* Toolbar */}
            <div className="flex items-center gap-3 px-4 py-2 border-b border-gray-700 bg-gray-800">
              <span className="text-sm font-medium">
                {activeFile}
                {dirty && <span className="text-yellow-400 ml-1">*</span>}
              </span>
              <div className="flex-1" />
              <button
                onClick={saveFile}
                disabled={saving || !dirty}
                className="px-3 py-1 text-xs bg-blue-600 hover:bg-blue-500 disabled:opacity-40 rounded"
              >
                {saving ? 'Saving...' : 'Save'}
              </button>
              <button
                onClick={deleteFile}
                className="px-3 py-1 text-xs bg-red-700 hover:bg-red-600 rounded"
              >
                Delete
              </button>
            </div>

            {/* Monaco Editor */}
            <div className="flex-1">
              <Editor
                language="lua"
                theme="vs-dark"
                value={content}
                onChange={(value) => {
                  setContent(value ?? '');
                  setDirty(true);
                }}
                onMount={handleEditorMount}
                options={{
                  fontSize: 14,
                  minimap: { enabled: false },
                  scrollBeyondLastLine: false,
                  wordWrap: 'on',
                  tabSize: 4,
                  insertSpaces: true,
                  automaticLayout: true,
                }}
              />
            </div>
          </>
        ) : (
          <div className="flex items-center justify-center h-full text-gray-500">
            <p>Select a script to edit</p>
          </div>
        )}
      </div>
    </div>
  );
}
