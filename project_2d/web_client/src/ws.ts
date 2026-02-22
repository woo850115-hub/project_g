import type { ClientMessage, ServerMessage } from "./protocol";

export class GameConnection {
  private ws: WebSocket | null = null;

  onMessage: ((msg: ServerMessage) => void) | null = null;
  onClose: (() => void) | null = null;
  onOpen: (() => void) | null = null;

  connect(url: string): void {
    this.ws = new WebSocket(url);

    this.ws.onopen = () => {
      this.onOpen?.();
    };

    this.ws.onmessage = (ev: MessageEvent) => {
      try {
        const msg = JSON.parse(ev.data as string) as ServerMessage;
        this.onMessage?.(msg);
      } catch {
        console.warn("Failed to parse server message:", ev.data);
      }
    };

    this.ws.onclose = () => {
      this.ws = null;
      this.onClose?.();
    };

    this.ws.onerror = () => {
      // onclose will fire after onerror
    };
  }

  send(msg: ClientMessage): void {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(msg));
    }
  }

  disconnect(): void {
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
  }

  get isConnected(): boolean {
    return this.ws !== null && this.ws.readyState === WebSocket.OPEN;
  }
}
