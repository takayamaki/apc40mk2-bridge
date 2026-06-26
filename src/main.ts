import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

interface MidiEvent {
  direction: "in" | "out";
  bytes: number[];
  timestamp_us: number;
}

const app = document.getElementById("app")!;
app.innerHTML = `
  <h1>APC40mk2 Bridge - Debug Monitor</h1>
  <div id="status">Waiting...</div>
  <div id="log" style="font-family: monospace; font-size: 13px; line-height: 1.4;"></div>
`;

const logEl = document.getElementById("log")!;
const statusEl = document.getElementById("status")!;

invoke<string>("get_status").then((s) => {
  statusEl.textContent = s;
});

function formatBytes(bytes: number[]): string {
  return bytes
    .map((b) => b.toString(16).padStart(2, "0").toUpperCase())
    .join(" ");
}

function formatTimestamp(us: number): string {
  const sec = (us / 1_000_000).toFixed(3);
  return `${sec}s`;
}

function appendLog(msg: string) {
  const line = document.createElement("div");
  line.textContent = msg;
  logEl.prepend(line);
  if (logEl.children.length > 500) {
    logEl.removeChild(logEl.lastChild!);
  }
}

listen<MidiEvent>("midi-event", (event) => {
  const { direction, bytes, timestamp_us } = event.payload;
  const arrow = direction === "in" ? "<<" : ">>";
  const ts = formatTimestamp(timestamp_us);
  appendLog(`[${ts}] ${arrow} ${formatBytes(bytes)}`);
});

listen<string>("bridge-status", (event) => {
  statusEl.textContent = event.payload;
});

listen<string[]>("stream-test-log", (event) => {
  for (const line of event.payload) {
    appendLog(`[stream-test] ${line}`);
  }
});
