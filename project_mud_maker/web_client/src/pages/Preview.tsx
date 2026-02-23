import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import '@xterm/xterm/css/xterm.css';
import { AnsiUp } from 'ansi_up';
import { serverApi, generateAllApi } from '../api/client';
import type { ServerStatus } from '../types/api';
import { Tooltip } from '../components/Tooltip';

export function Preview() {
  const [status, setStatus] = useState<ServerStatus>({ running: false });
  const [starting, setStarting] = useState(false);
  const [stopping, setStopping] = useState(false);
  const [logs, setLogs] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);

  // ANSI to HTML converter (inline styles for color rendering)
  const ansiUp = useMemo(() => new AnsiUp(), []);

  // Terminal refs
  const termRef = useRef<HTMLDivElement>(null);
  const xtermRef = useRef<Terminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const previewWsRef = useRef<WebSocket | null>(null);
  const logWsRef = useRef<WebSocket | null>(null);
  const logsEndRef = useRef<HTMLDivElement>(null);

  // Poll server status
  const refreshStatus = useCallback(async () => {
    try {
      const s = await serverApi.status();
      setStatus(s);
    } catch {
      // ignore
    }
  }, []);

  useEffect(() => {
    refreshStatus();
    const interval = setInterval(refreshStatus, 3000);
    return () => clearInterval(interval);
  }, [refreshStatus]);

  // Initialize xterm
  useEffect(() => {
    if (!termRef.current) return;

    const term = new Terminal({
      theme: {
        background: '#1a1a2e',
        foreground: '#e0e0e0',
        cursor: '#00ff88',
        cursorAccent: '#1a1a2e',
      },
      fontSize: 14,
      fontFamily: '"Cascadia Code", "Fira Code", monospace',
      cursorBlink: true,
      convertEol: true,
    });

    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(termRef.current);
    fit.fit();

    xtermRef.current = term;
    fitRef.current = fit;

    term.writeln('\x1b[36m=== MUD \uAC8C\uC784 \uBA54\uC774\uCEE4 \uBBF8\uB9AC\uBCF4\uAE30 ===\x1b[0m');
    term.writeln('\uC11C\uBC84\uB97C \uC2DC\uC791\uD558\uACE0 \uC5F0\uACB0\uD558\uC5EC \uD50C\uB808\uC774\uD558\uC138\uC694.\r\n');

    // Handle resize
    const observer = new ResizeObserver(() => fit.fit());
    observer.observe(termRef.current);

    return () => {
      observer.disconnect();
      term.dispose();
      xtermRef.current = null;
      fitRef.current = null;
    };
  }, []);

  // Connect log WebSocket when server starts
  useEffect(() => {
    if (!status.running) {
      if (logWsRef.current) {
        logWsRef.current.close();
        logWsRef.current = null;
      }
      return;
    }

    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const ws = new WebSocket(`${protocol}//${window.location.host}/ws/logs`);
    logWsRef.current = ws;

    ws.onmessage = (e) => {
      try {
        const data = JSON.parse(e.data);
        if (data.type === 'log') {
          setLogs((prev) => {
            const next = [...prev, data.text];
            return next.length > 500 ? next.slice(-500) : next;
          });
        }
      } catch {
        // ignore
      }
    };

    return () => {
      ws.close();
      logWsRef.current = null;
    };
  }, [status.running]);

  // Auto-scroll logs
  useEffect(() => {
    logsEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [logs]);

  // Connect preview terminal WebSocket
  const connectTerminal = () => {
    const term = xtermRef.current;
    if (!term) return;

    // Close existing connection
    if (previewWsRef.current) {
      previewWsRef.current.close();
      previewWsRef.current = null;
    }

    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const ws = new WebSocket(`${protocol}//${window.location.host}/ws/preview`);
    previewWsRef.current = ws;

    term.writeln('\x1b[33mMUD \uC11C\uBC84\uC5D0 \uC5F0\uACB0 \uC911...\x1b[0m');

    let inputBuffer = '';

    ws.onopen = () => {
      term.writeln('\x1b[32m\uC5F0\uACB0\uB428!\x1b[0m\r\n');
    };

    ws.onmessage = (e) => {
      try {
        const data = JSON.parse(e.data);
        if (data.type === 'output') {
          term.write(data.text);
        } else if (data.type === 'error') {
          term.writeln(`\x1b[31m${data.text}\x1b[0m`);
        }
      } catch {
        // ignore
      }
    };

    ws.onclose = () => {
      term.writeln('\r\n\x1b[31mMUD \uC11C\uBC84\uC640\uC758 \uC5F0\uACB0\uC774 \uB04A\uACBC\uC2B5\uB2C8\uB2E4.\x1b[0m');
      previewWsRef.current = null;
    };

    // Handle terminal input
    term.onData((data) => {
      if (!previewWsRef.current || previewWsRef.current.readyState !== WebSocket.OPEN) return;

      if (data === '\r') {
        // Enter pressed — send the buffered input
        term.write('\r\n');
        previewWsRef.current.send(JSON.stringify({ type: 'input', text: inputBuffer }));
        inputBuffer = '';
      } else if (data === '\x7f') {
        // Backspace
        if (inputBuffer.length > 0) {
          inputBuffer = inputBuffer.slice(0, -1);
          term.write('\b \b');
        }
      } else if (data >= ' ') {
        // Printable character
        inputBuffer += data;
        term.write(data);
      }
    });
  };

  // Disconnect terminal
  const disconnectTerminal = () => {
    if (previewWsRef.current) {
      previewWsRef.current.close();
      previewWsRef.current = null;
    }
  };

  // Server controls
  const startServer = async () => {
    setStarting(true);
    try {
      await serverApi.start();
      // Wait a bit for server to initialize
      await new Promise((r) => setTimeout(r, 2000));
      await refreshStatus();
    } catch (e) {
      setError(e instanceof Error ? e.message : '\uC11C\uBC84 \uC2DC\uC791 \uC2E4\uD328');
    } finally {
      setStarting(false);
    }
  };

  const stopServer = async () => {
    setStopping(true);
    disconnectTerminal();
    try {
      await serverApi.stop();
      await refreshStatus();
    } catch (e) {
      setError(e instanceof Error ? e.message : '\uC11C\uBC84 \uC815\uC9C0 \uC2E4\uD328');
    } finally {
      setStopping(false);
    }
  };

  const restartServer = async () => {
    setStopping(true);
    disconnectTerminal();
    try {
      await serverApi.restart();
      await new Promise((r) => setTimeout(r, 2000));
      await refreshStatus();
    } catch (e) {
      setError(e instanceof Error ? e.message : '서버 재시작 실패');
    } finally {
      setStopping(false);
    }
  };

  const generateAllAndRestart = async () => {
    setStopping(true);
    disconnectTerminal();
    try {
      await generateAllApi.generateAll();
      await serverApi.restart();
      await new Promise((r) => setTimeout(r, 2000));
      await refreshStatus();
    } catch (e) {
      setError(e instanceof Error ? e.message : '전체 생성+재시작 실패');
    } finally {
      setStopping(false);
    }
  };

  return (
    <div className="flex flex-col h-full">
      {/* Error toast */}
      {error && (
        <div className="fixed top-4 right-4 bg-red-600 text-white px-4 py-2 rounded shadow-lg z-50">
          {error}
          <button className="ml-2 font-bold" onClick={() => setError(null)}>x</button>
        </div>
      )}

      {/* Server controls bar */}
      <div className="flex items-center gap-3 px-4 py-2 border-b border-gray-700 bg-gray-800">
        <div className="flex items-center gap-2">
          <span
            className={`w-2.5 h-2.5 rounded-full ${
              status.running ? 'bg-green-400' : 'bg-gray-500'
            }`}
          />
          <span className="text-sm">
            {status.running ? `\uC2E4\uD589 \uC911 (PID ${status.pid})` : '\uC815\uC9C0\uB428'}
          </span>
        </div>
        <div className="flex gap-2 ml-4">
          {!status.running ? (
            <Tooltip text="MUD 서버를 시작하여 게임을 미리보기합니다">
              <button
                onClick={startServer}
                disabled={starting}
                className="px-3 py-1 text-xs bg-green-700 hover:bg-green-600 disabled:opacity-50 rounded"
              >
                {starting ? '\uC2DC\uC791 \uC911...' : '\uC11C\uBC84 \uC2DC\uC791'}
              </button>
            </Tooltip>
          ) : (
            <>
              <Tooltip text="MUD 서버에 연결하여 게임 터미널을 엽니다">
                <button
                  onClick={connectTerminal}
                  className="px-3 py-1 text-xs bg-blue-600 hover:bg-blue-500 rounded"
                >
                  연결
                </button>
              </Tooltip>
              <button
                onClick={disconnectTerminal}
                className="px-3 py-1 text-xs bg-gray-600 hover:bg-gray-500 rounded"
              >
                연결 해제
              </button>
              <Tooltip text="모든 Lua 생성 후 서버 재시작">
                <button
                  onClick={generateAllAndRestart}
                  disabled={stopping}
                  className="px-3 py-1 text-xs bg-green-600 hover:bg-green-500 disabled:opacity-50 rounded"
                >
                  전체 생성+재시작
                </button>
              </Tooltip>
              <button
                onClick={restartServer}
                disabled={stopping}
                className="px-3 py-1 text-xs bg-yellow-700 hover:bg-yellow-600 disabled:opacity-50 rounded"
              >
                재시작
              </button>
              <button
                onClick={stopServer}
                disabled={stopping}
                className="px-3 py-1 text-xs bg-red-700 hover:bg-red-600 disabled:opacity-50 rounded"
              >
                {stopping ? '\uC815\uC9C0 \uC911...' : '\uC815\uC9C0'}
              </button>
            </>
          )}
        </div>
      </div>

      {/* Main content: terminal + logs side by side */}
      <div className="flex-1 flex overflow-hidden">
        {/* MUD Terminal */}
        <div className="flex-1 flex flex-col border-r border-gray-700">
          <div className="px-3 py-1.5 text-xs text-gray-400 border-b border-gray-700 bg-gray-800">
            MUD 터미널
          </div>
          <div ref={termRef} className="flex-1" />
        </div>

        {/* Log viewer */}
        <div className="w-[400px] flex flex-col bg-gray-900">
          <div className="px-3 py-1.5 text-xs text-gray-400 border-b border-gray-700 bg-gray-800 flex items-center justify-between">
            <span>서버 로그</span>
            <button
              onClick={() => setLogs([])}
              className="text-gray-500 hover:text-gray-300"
            >
              지우기
            </button>
          </div>
          <div className="flex-1 overflow-y-auto p-2 font-mono text-xs text-gray-300">
            {logs.length === 0 ? (
              <p className="text-gray-600">로그가 없습니다. 서버를 시작하면 출력을 볼 수 있습니다.</p>
            ) : (
              logs.map((line, i) => (
                <div
                  key={i}
                  className="py-0.5 leading-relaxed"
                  dangerouslySetInnerHTML={{ __html: ansiUp.ansi_to_html(line) }}
                />
              ))
            )}
            <div ref={logsEndRef} />
          </div>
        </div>
      </div>
    </div>
  );
}
