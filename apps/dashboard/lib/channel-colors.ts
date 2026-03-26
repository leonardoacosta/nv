/**
 * Deterministic channel accent color hashing.
 *
 * Known channels (telegram, discord, slack, cli, api) return their brand colors.
 * Unknown/dynamic channels are hashed into a curated 8-color palette designed
 * for legibility against dark `ds-gray-100` / `ds-gray-200` backgrounds.
 */

// 8 curated accent colors for unknown channels — tested against dark theme
const ACCENT_PALETTE = [
  "#6366f1", // indigo-500
  "#f59e0b", // amber-500
  "#10b981", // emerald-500
  "#ef4444", // red-500
  "#8b5cf6", // violet-500
  "#06b6d4", // cyan-500
  "#f97316", // orange-500
  "#ec4899", // pink-500
] as const;

// Brand colors for known channels (hex values)
const BRAND_COLORS: Record<string, string> = {
  telegram: "#229ED9",
  discord: "#5865F2",
  slack: "#E01E5A",
  cli: "#a0a0a0", // ds-gray-1000 equivalent
  api: "#b91c1c", // red-700
};

/**
 * Simple string hash (djb2) returning a non-negative integer.
 */
function djb2(str: string): number {
  let hash = 5381;
  for (let i = 0; i < str.length; i++) {
    hash = (hash * 33) ^ str.charCodeAt(i);
  }
  return hash >>> 0; // ensure unsigned
}

/**
 * Returns a hex color string for a given channel name.
 * Known channels get their brand color; unknown channels get a
 * deterministic color from the 8-color palette.
 */
export function channelAccentColor(name: string): string {
  const key = name.toLowerCase();
  if (key in BRAND_COLORS) return BRAND_COLORS[key]!;
  return ACCENT_PALETTE[djb2(key) % ACCENT_PALETTE.length]!;
}
