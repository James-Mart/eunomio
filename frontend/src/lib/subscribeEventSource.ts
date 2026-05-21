export type ConnectionStatus = "connecting" | "open" | "closed";

export type SubscribeEventSourceOptions = {
  url: string;
  onMessage: (event: MessageEvent) => void;
  onConnectionChange?: (status: ConnectionStatus) => void;
};

export function subscribeEventSource({
  url,
  onMessage,
  onConnectionChange,
}: SubscribeEventSourceOptions): () => void {
  let source: EventSource | null = null;
  let reconnectTimer: number | null = null;
  let attempt = 0;
  let stopped = false;

  const connect = () => {
    if (stopped) return;
    onConnectionChange?.("connecting");
    source = new EventSource(url);
    source.onopen = () => {
      attempt = 0;
      onConnectionChange?.("open");
    };
    source.onmessage = onMessage;
    source.onerror = () => {
      if (stopped) return;
      onConnectionChange?.("closed");
      source?.close();
      source = null;
      const delay = backoffMs(attempt++);
      reconnectTimer = window.setTimeout(connect, delay);
    };
  };

  connect();

  return () => {
    stopped = true;
    if (reconnectTimer != null) window.clearTimeout(reconnectTimer);
    source?.close();
  };
}

function backoffMs(attempt: number): number {
  const capped = Math.min(attempt, 5);
  return Math.min(1000 * 2 ** capped, 30_000);
}
