<script lang="ts">
  import { onMount } from "svelte";

  interface TapMessage {
    id: string;
    topic: string;
    project: string;
    data: unknown;
    attributes: Record<string, string>;
    publish_time: string;
    tapped_at: string;
  }

  let messages = $state<TapMessage[]>([]);
  let paused = $state(false);
  let filter = $state("");
  let selectedTopic = $state<string | null>(null);
  let expandedIds = $state<Record<string, boolean>>({});
  let connected = $state(false);
  let messagesPerSec = $state(0);
  let darkMode = $state(false);

  let recentTimestamps: number[] = [];
  let eventSource: EventSource | null = null;

  let topics = $derived([...new Set(messages.map((m) => m.topic))].sort());

  let filteredMessages = $derived(
    messages.filter((m) => {
      if (selectedTopic && m.topic !== selectedTopic) return false;
      if (filter) {
        const q = filter.toLowerCase();
        return (
          m.topic.toLowerCase().includes(q) ||
          JSON.stringify(m.data).toLowerCase().includes(q)
        );
      }
      return true;
    }),
  );

  function updateThroughput() {
    const now = Date.now();
    recentTimestamps = recentTimestamps.filter((t) => now - t < 5000);
    messagesPerSec = Math.round(recentTimestamps.length / 5);
  }

  onMount(() => {
    const saved = localStorage.getItem("pubsub-tap-theme");
    if (saved) darkMode = saved === "dark";
    connect();
    const interval = setInterval(updateThroughput, 1000);
    return () => {
      clearInterval(interval);
      eventSource?.close();
    };
  });

  function toggleTheme() {
    darkMode = !darkMode;
    localStorage.setItem("pubsub-tap-theme", darkMode ? "dark" : "light");
  }

  function connect() {
    eventSource = new EventSource("/api/events");
    eventSource.onopen = () => {
      connected = true;
    };
    eventSource.onerror = () => {
      connected = false;
    };
    eventSource.addEventListener("message", (e) => {
      if (paused) return;
      const msg: TapMessage = JSON.parse(e.data);
      messages.push(msg);
      recentTimestamps.push(Date.now());

      if (messages.length > 1000) {
        messages.splice(0, messages.length - 500);
      }

      requestAnimationFrame(() => {
        const el = document.getElementById("message-list");
        if (el) el.scrollTop = el.scrollHeight;
      });
    });
  }

  function toggleExpanded(id: string) {
    if (expandedIds[id]) {
      delete expandedIds[id];
    } else {
      expandedIds[id] = true;
    }
  }

  function clear() {
    messages.length = 0;
    recentTimestamps = [];
  }

  function formatTime(iso: string): string {
    try {
      return new Date(iso).toLocaleTimeString("en-US", {
        hour12: false,
        fractionalSecondDigits: 3,
      });
    } catch {
      return iso;
    }
  }

  const topicColors = new Map<string, string>();
  const palette = [
    "#0969da",
    "#1a7f37",
    "#9a6700",
    "#cf222e",
    "#8250df",
    "#0e8a6d",
    "#bf3989",
    "#0550ae",
    "#116329",
    "#953800",
  ];

  function topicColor(topic: string): string {
    if (!topicColors.has(topic)) {
      topicColors.set(topic, palette[topicColors.size % palette.length]);
    }
    return topicColors.get(topic)!;
  }

  function formatJson(data: unknown): string {
    if (typeof data === "string") return data;
    return JSON.stringify(data, null, 2);
  }

  function previewJson(data: unknown): string {
    const str = typeof data === "string" ? data : JSON.stringify(data);
    return str.length > 120 ? str.slice(0, 120) + "\u2026" : str;
  }
</script>

<div class="app" class:dark={darkMode}>
  <header>
    <div class="brand">
      <span class="logo">&#9889;</span>
      <h1>pubsub-tap</h1>
    </div>
    <div class="stats">
      <span class="status" class:connected
        >{connected ? "Connected" : "Disconnected"}</span
      >
      <span class="stat">{messages.length} msgs</span>
      <span class="stat">{messagesPerSec}/sec</span>
      <button class="theme-toggle" onclick={toggleTheme} title="Toggle theme">
        {darkMode ? "\u2600\ufe0f" : "\u263e"}
      </button>
    </div>
  </header>

  <div class="toolbar">
    <div class="topics">
      <button
        class="topic-pill"
        class:active={!selectedTopic}
        onclick={() => (selectedTopic = null)}
      >
        All
      </button>
      {#each topics as topic}
        <button
          class="topic-pill"
          class:active={selectedTopic === topic}
          style="--topic-color: {topicColor(topic)}"
          onclick={() =>
            (selectedTopic = selectedTopic === topic ? null : topic)}
        >
          {topic}
        </button>
      {/each}
    </div>
    <div class="actions">
      <input
        type="text"
        placeholder="Filter..."
        bind:value={filter}
        class="search"
      />
      <button
        class="btn"
        class:active={paused}
        onclick={() => (paused = !paused)}
      >
        {paused ? "\u25b6 Resume" : "\u23f8 Pause"}
      </button>
      <button class="btn" onclick={clear}>Clear</button>
    </div>
  </div>

  <div class="message-list" id="message-list">
    {#if filteredMessages.length === 0}
      <div class="empty">
        {#if messages.length === 0}
          <p>Waiting for messages&hellip;</p>
          <p class="muted">
            Messages published to any Pub/Sub topic will appear here in real
            time.
          </p>
        {:else}
          <p>No messages match your filter.</p>
        {/if}
      </div>
    {:else}
      {#each filteredMessages as msg (msg.id + msg.tapped_at)}
        <button class="message" onclick={() => toggleExpanded(msg.id)}>
          <div class="message-header">
            <span class="time">{formatTime(msg.tapped_at)}</span>
            <span
              class="topic-badge"
              style="--topic-color: {topicColor(msg.topic)}"
            >
              {msg.topic}
            </span>
            {#if Object.keys(msg.attributes).length > 0}
              <span class="attr-count"
                >{Object.keys(msg.attributes).length} attrs</span
              >
            {/if}
            <span class="preview">{previewJson(msg.data)}</span>
          </div>
          {#if expandedIds[msg.id]}
            <div class="message-detail">
              <div class="detail-section">
                <span class="detail-label">ID</span>
                <span class="detail-value">{msg.id}</span>
              </div>
              <div class="detail-section">
                <span class="detail-label">Published</span>
                <span class="detail-value">{msg.publish_time}</span>
              </div>
              {#if Object.keys(msg.attributes).length > 0}
                <div class="detail-section">
                  <span class="detail-label">Attributes</span>
                  <pre class="detail-value">{JSON.stringify(
                      msg.attributes,
                      null,
                      2,
                    )}</pre>
                </div>
              {/if}
              <div class="detail-section">
                <span class="detail-label">Data</span>
                <pre class="detail-value payload">{formatJson(msg.data)}</pre>
              </div>
            </div>
          {/if}
        </button>
      {/each}
    {/if}
  </div>
</div>

<style>
  /* ── Light theme (default) ── */
  .app {
    --bg: #ffffff;
    --bg-surface: #f6f8fa;
    --bg-inset: #f0f2f5;
    --border: #d0d7de;
    --border-subtle: #e8ebef;
    --text: #1f2328;
    --text-muted: #656d76;
    --text-faint: #8b949e;
    --accent: #0969da;
    --status-ok: #1a7f37;
    --status-ok-bg: rgba(26, 127, 55, 0.1);
    --status-err: #cf222e;
    --status-err-bg: rgba(207, 34, 46, 0.1);
    --hover-bg: rgba(9, 105, 218, 0.04);
    --pill-active-text: #ffffff;
    --payload-color: #0550ae;
    --warn-bg: #9a6700;
    --warn-text: #1f2328;
  }

  /* ── Dark theme ── */
  .app.dark {
    --bg: #0d1117;
    --bg-surface: #161b22;
    --bg-inset: #0d1117;
    --border: #30363d;
    --border-subtle: #21262d;
    --text: #e6edf3;
    --text-muted: #8b949e;
    --text-faint: #484f58;
    --accent: #58a6ff;
    --status-ok: #3fb950;
    --status-ok-bg: rgba(63, 185, 80, 0.15);
    --status-err: #f85149;
    --status-err-bg: rgba(248, 81, 73, 0.15);
    --hover-bg: rgba(88, 166, 255, 0.04);
    --pill-active-text: #0d1117;
    --payload-color: #79c0ff;
    --warn-bg: #d29922;
    --warn-text: #0d1117;
  }

  :global(body) {
    margin: 0;
    padding: 0;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui,
      sans-serif;
    font-size: 14px;
  }

  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--bg);
    color: var(--text);
  }

  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 20px;
    background: var(--bg-surface);
    border-bottom: 1px solid var(--border);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .logo {
    font-size: 20px;
  }

  h1 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
  }

  .stats {
    display: flex;
    align-items: center;
    gap: 16px;
  }

  .status {
    padding: 2px 8px;
    border-radius: 12px;
    font-size: 12px;
    background: var(--status-err-bg);
    color: var(--status-err);
  }

  .status.connected {
    background: var(--status-ok-bg);
    color: var(--status-ok);
  }

  .stat {
    font-size: 13px;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }

  .theme-toggle {
    background: none;
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 2px 8px;
    font-size: 16px;
    cursor: pointer;
    line-height: 1;
  }

  .toolbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 8px 20px;
    background: var(--bg-surface);
    border-bottom: 1px solid var(--border);
    gap: 12px;
    flex-wrap: wrap;
  }

  .topics {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }

  .topic-pill {
    padding: 3px 10px;
    border-radius: 12px;
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-muted);
    font-size: 12px;
    cursor: pointer;
    transition: all 0.15s;
  }

  .topic-pill:hover {
    border-color: var(--accent);
    color: var(--text);
  }

  .topic-pill.active {
    background: var(--topic-color, var(--accent));
    border-color: var(--topic-color, var(--accent));
    color: var(--pill-active-text);
    font-weight: 500;
  }

  .actions {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .search {
    padding: 4px 10px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text);
    font-size: 13px;
    outline: none;
    width: 180px;
  }

  .search:focus {
    border-color: var(--accent);
  }

  .btn {
    padding: 4px 12px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--bg-surface);
    color: var(--text);
    font-size: 12px;
    cursor: pointer;
    transition: all 0.15s;
  }

  .btn:hover {
    background: var(--bg-inset);
  }

  .btn.active {
    background: var(--warn-bg);
    border-color: var(--warn-bg);
    color: var(--warn-text);
  }

  .message-list {
    flex: 1;
    overflow-y: auto;
    padding: 8px 0;
  }

  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: var(--text-muted);
  }

  .empty p {
    margin: 4px;
  }

  .muted {
    font-size: 13px;
    color: var(--text-faint);
  }

  .message {
    display: block;
    width: 100%;
    text-align: left;
    padding: 6px 20px;
    cursor: pointer;
    border: none;
    border-bottom: 1px solid var(--border-subtle);
    background: transparent;
    color: inherit;
    font: inherit;
    transition: background 0.1s;
  }

  .message:hover {
    background: var(--hover-bg);
  }

  .message-header {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 28px;
  }

  .time {
    font-family: "SF Mono", "Fira Code", "Cascadia Code", monospace;
    font-size: 12px;
    color: var(--text-faint);
    flex-shrink: 0;
  }

  .topic-badge {
    font-size: 11px;
    font-weight: 600;
    padding: 1px 8px;
    border-radius: 10px;
    background: color-mix(in srgb, var(--topic-color) 15%, transparent);
    color: var(--topic-color);
    flex-shrink: 0;
  }

  .attr-count {
    font-size: 11px;
    color: var(--text-muted);
    background: var(--bg-inset);
    padding: 1px 6px;
    border-radius: 4px;
    flex-shrink: 0;
  }

  .preview {
    font-family: "SF Mono", "Fira Code", "Cascadia Code", monospace;
    font-size: 12px;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }

  .message-detail {
    margin-top: 8px;
    padding: 10px;
    background: var(--bg-inset);
    border-radius: 6px;
    border: 1px solid var(--border-subtle);
  }

  .detail-section {
    margin-bottom: 8px;
  }

  .detail-section:last-child {
    margin-bottom: 0;
  }

  .detail-label {
    display: block;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
    margin-bottom: 3px;
  }

  .detail-value {
    font-family: "SF Mono", "Fira Code", "Cascadia Code", monospace;
    font-size: 12px;
    color: var(--text);
    margin: 0;
    white-space: pre-wrap;
    word-break: break-all;
  }

  .payload {
    color: var(--payload-color);
  }
</style>
