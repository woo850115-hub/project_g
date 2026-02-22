import { GameState } from "./state";
import { GameConnection } from "./ws";
import { InputHandler } from "./input";
import { GameRenderer } from "./renderer";
import type { ServerMessage } from "./protocol";

const state = new GameState();
const connection = new GameConnection();
const input = new InputHandler(connection);
const renderer = new GameRenderer();

// DOM elements
const loginOverlay = document.getElementById("login-overlay") as HTMLDivElement;
const nameInput = document.getElementById("name-input") as HTMLInputElement;
const connectBtn = document.getElementById("connect-btn") as HTMLButtonElement;
const statusBar = document.getElementById("status-bar") as HTMLDivElement;
const statusText = document.getElementById("status-text") as HTMLSpanElement;

function getWsUrl(): string {
  const proto = location.protocol === "https:" ? "wss:" : "ws:";
  return `${proto}//${location.host}/ws`;
}

function showLogin(): void {
  loginOverlay.style.display = "flex";
  statusBar.style.display = "none";
  nameInput.focus();
}

function hideLogin(): void {
  loginOverlay.style.display = "none";
  statusBar.style.display = "block";
}

function updateStatus(text: string): void {
  statusText.textContent = text;
}

let pendingName = "";

function doConnect(): void {
  const name = nameInput.value.trim();
  if (!name) return;
  pendingName = name;

  connectBtn.disabled = true;
  connectBtn.textContent = "Connecting...";

  connection.connect(getWsUrl());
}

// Connection callbacks
connection.onOpen = () => {
  connection.send({ type: "connect", name: pendingName });
};

connection.onMessage = (msg: ServerMessage) => {
  switch (msg.type) {
    case "welcome":
      state.connected = true;
      state.sessionId = msg.session_id;
      state.selfEntityId = msg.entity_id;
      state.tick = msg.tick;
      state.gridConfig = msg.grid_config;

      hideLogin();
      updateStatus(
        `Connected as ${pendingName} | Grid: ${msg.grid_config.width}x${msg.grid_config.height}`
      );
      renderer.setGameState(state);
      input.start();
      break;

    case "state_delta":
      state.tick = msg.tick;
      if (msg.entered) state.applyEntered(msg.entered);
      if (msg.moved) state.applyMoved(msg.moved);
      if (msg.left) state.applyLeft(msg.left);
      renderer.syncEntities(state);
      break;

    case "error":
      console.error("Server error:", msg.message);
      break;

    case "pong":
      break;
  }
};

connection.onClose = () => {
  state.reset();
  renderer.clear();
  input.stop();
  connectBtn.disabled = false;
  connectBtn.textContent = "Connect";
  showLogin();
};

// UI event listeners
connectBtn.addEventListener("click", doConnect);
nameInput.addEventListener("keydown", (e: KeyboardEvent) => {
  if (e.key === "Enter") doConnect();
});

// Initialize renderer
async function main(): Promise<void> {
  const container = document.getElementById("canvas-container") as HTMLDivElement;
  await renderer.init(container);
  showLogin();
}

main();
