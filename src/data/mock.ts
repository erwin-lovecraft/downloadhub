import type {
  ActivityLogEntry,
  Agent,
  AgentProposal,
  FormatOption,
  HistoryItem,
  QueueItem,
  Video,
} from "@/types";

/**
 * Placeholder data ported from the DownloadHub.dc.html design prototype.
 * This module is the single place Phase 1 steps 2-8 touch when swapping a
 * mock for a real Tauri `invoke` call (YouTube Data API search, queue
 * persistence, etc.) — the rest of the UI should not need to change shape.
 */

const LOREM_TITLES = [
  "Lorem Ipsum Dolor Sit Amet Consectetur",
  "Adipiscing Elit Sed Do Eiusmod Tempor",
  "Incididunt Ut Labore Et Dolore Magna",
  "Aliqua Ut Enim Ad Minim Veniam",
  "Quis Nostrud Exercitation Ullamco Laboris",
  "Nisi Ut Aliquip Ex Ea Commodo",
  "Duis Aute Irure Dolor In Reprehenderit",
  "Voluptate Velit Esse Cillum Dolore",
  "Eu Fugiat Nulla Pariatur Excepteur",
  "Sint Occaecat Cupidatat Non Proident",
  "Sunt In Culpa Qui Officia",
  "Deserunt Mollit Anim Id Est Laborum",
];

const CHANNELS = [
  "Consectetur Studio",
  "Tempor Labs",
  "Magna Aliqua Media",
  "Nostrud Films",
  "Commodo Creators",
  "Reprehenderit TV",
  "Voluptate Vision",
  "Fugiat Network",
  "Proident Productions",
  "Culpa Collective",
  "Laborum Media",
  "Elit Broadcasting",
];

const DURATIONS = [
  "12:34",
  "4:07",
  "1:02:15",
  "8:41",
  "0:47",
  "23:10",
  "3:58",
  "15:22",
  "6:03",
  "44:12",
  "2:11",
  "9:59",
];

const VIEWS = [
  "1.2M views",
  "384K views",
  "12M views",
  "95K views",
  "2.3M views",
  "640K views",
  "78K views",
  "3.1M views",
  "210K views",
  "5.4M views",
  "41K views",
  "999K views",
];

const UPLOADED = [
  "3 days ago",
  "1 week ago",
  "2 months ago",
  "5 hours ago",
  "1 year ago",
  "4 days ago",
  "6 months ago",
  "2 weeks ago",
  "11 months ago",
  "1 day ago",
  "3 weeks ago",
  "8 months ago",
];

const SUBS = [
  "1.2M subscribers",
  "384K subscribers",
  "12M subscribers",
  "95K subscribers",
  "2.3M subscribers",
  "640K subscribers",
  "78K subscribers",
  "3.1M subscribers",
  "210K subscribers",
  "5.4M subscribers",
  "41K subscribers",
  "999K subscribers",
];

const DESC =
  "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua, ut enim ad minim veniam quis nostrud.";

const PALETTES: [string, string][] = [
  ["#D8E4EF", "#C7D8E8"],
  ["#E8DEF0", "#DACBEA"],
  ["#DDEEE1", "#C9E3CF"],
  ["#F0E4D8", "#E6D2BF"],
  ["#E0E6F0", "#CBD6E8"],
  ["#EFE0E6", "#E3C9D3"],
];

export function thumbGradient(i: number): string {
  const [a, b] = PALETTES[i % PALETTES.length];
  return `repeating-linear-gradient(135deg, ${a} 0px, ${a} 12px, ${b} 12px 24px)`;
}

export const ALL_VIDEOS: Video[] = LOREM_TITLES.map((title, i) => ({
  id: `v${i}`,
  title,
  channel: CHANNELS[i],
  duration: DURATIONS[i],
  views: VIEWS[i],
  uploaded: UPLOADED[i],
  thumbGradient: thumbGradient(i),
  subs: SUBS[i],
  desc: DESC,
}));

export const FORMAT_TABLE: FormatOption[] = [
  { key: "p2160", res: "2160p (4K)", type: "Video only", itag: "401", size: "1.8 GB" },
  { key: "p1080", res: "1080p", type: "Video + Audio", itag: "137+140", size: "845 MB" },
  { key: "p720", res: "720p", type: "Video + Audio", itag: "22", size: "480 MB" },
  { key: "p360", res: "360p", type: "Video + Audio", itag: "18", size: "210 MB" },
  { key: "audio", res: "Audio only (M4A)", type: "Audio only", itag: "140", size: "9.6 MB" },
];

export const QUEUE_SEED: QueueItem[] = [
  {
    id: "1",
    title: LOREM_TITLES[0],
    channel: CHANNELS[0],
    thumbGradient: thumbGradient(0),
    format: "1080p · MP4",
    status: "downloading",
    progress: 42,
    speed: "3.4 MB/s",
    eta: "4 min",
  },
  {
    id: "2",
    title: LOREM_TITLES[1],
    channel: CHANNELS[1],
    thumbGradient: thumbGradient(1),
    format: "720p · MP4",
    status: "queued",
    progress: 0,
  },
  {
    id: "3",
    title: LOREM_TITLES[2],
    channel: CHANNELS[2],
    thumbGradient: thumbGradient(2),
    format: "1080p · MP4",
    status: "paused",
    progress: 67,
  },
  {
    id: "4",
    title: LOREM_TITLES[3],
    channel: CHANNELS[3],
    thumbGradient: thumbGradient(3),
    format: "Audio only · M4A",
    status: "completed",
    progress: 100,
    size: "9.6 MB",
  },
  {
    id: "5",
    title: LOREM_TITLES[4],
    channel: CHANNELS[4],
    thumbGradient: thumbGradient(4),
    format: "1080p · MP4",
    status: "failed",
    progress: 18,
    error: "Network error",
  },
];

export const AGENT_PENDING_SEED: AgentProposal[] = [
  {
    id: "101",
    agentName: "Gemini",
    query: "lorem ipsum tutorial series",
    videoTitle: LOREM_TITLES[5],
    thumbGradient: thumbGradient(5),
    format: "1080p · MP4",
    time: "2 min ago",
  },
  {
    id: "102",
    agentName: "Codex",
    query: "consectetur adipiscing walkthrough",
    videoTitle: LOREM_TITLES[6],
    thumbGradient: thumbGradient(6),
    format: "720p · MP4",
    time: "18 min ago",
  },
];

export const AGENTS_SEED: Agent[] = [
  { key: "gemini", name: "Gemini", statusLabel: "Connected", permission: "manual" },
  { key: "codex", name: "Codex", statusLabel: "Connected", permission: "manual" },
];

export const ACTIVITY_LOG: ActivityLogEntry[] = [
  { id: "1", text: 'Gemini searched "lorem ipsum tutorial series"', time: "2 min ago" },
  { id: "2", text: `Gemini selected "${LOREM_TITLES[5]}"`, time: "2 min ago" },
  { id: "3", text: "Codex proposed download at 720p · MP4", time: "18 min ago" },
  { id: "4", text: "Codex download approved by you", time: "1 hour ago" },
  { id: "5", text: "Codex download completed", time: "55 min ago" },
];

export const HISTORY_ITEMS: HistoryItem[] = LOREM_TITLES.slice(7, 11).map((title, i) => ({
  id: `h${i}`,
  title,
  channel: CHANNELS[7 + i],
  thumbGradient: thumbGradient(7 + i),
  format: i % 2 === 0 ? "1080p · MP4" : "720p · MP4",
  path: `C:\\Users\\Guest\\Downloads\\${title.toLowerCase().replace(/[^a-z0-9]+/g, "-")}.mp4`,
  date: `Jul ${8 + i}, 2026`,
  size: i % 2 === 0 ? "845 MB" : "480 MB",
}));
