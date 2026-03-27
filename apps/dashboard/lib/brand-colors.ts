/**
 * Platform brand color map.
 * Platform brand colors are the one legitimate use of hardcoded hex in this codebase —
 * they are external brand identities and do not belong in the Geist design token system.
 * All other colors must use ds-* tokens or Tailwind-mapped status colors.
 */
export const PLATFORM_BRAND = {
  telegram: {
    bg: "bg-[#229ED9]/20",
    text: "text-[#229ED9]",
    border: "border-[#229ED9]/30",
    dot: "bg-[#229ED9]",
  },
  discord: {
    bg: "bg-[#5865F2]/20",
    text: "text-[#5865F2]",
    border: "border-[#5865F2]/30",
    dot: "bg-[#5865F2]",
  },
  slack: {
    bg: "bg-[#4A154B]/20",
    text: "text-[#E01E5A]",
    border: "border-[#E01E5A]/30",
    dot: "bg-[#E01E5A]",
  },
  cli: {
    bg: "bg-ds-gray-alpha-200",
    text: "text-ds-gray-1000",
    border: "border-ds-gray-alpha-400",
    dot: "bg-ds-gray-700",
  },
  api: {
    bg: "bg-red-700/20",
    text: "text-red-700",
    border: "border-red-700/30",
    dot: "bg-red-700",
  },
} as const;

export type PlatformKey = keyof typeof PLATFORM_BRAND;

const NEUTRAL_BRAND = {
  bg: "bg-ds-gray-alpha-200",
  text: "text-ds-gray-900",
  border: "border-ds-gray-alpha-400",
  dot: "bg-ds-gray-600",
} as const;

/**
 * Returns the brand color entry for a given channel/platform key.
 * Falls back to neutral ds-gray tokens for unknown platforms.
 */
export function getPlatformColor(
  channel: string,
): (typeof PLATFORM_BRAND)[PlatformKey] | typeof NEUTRAL_BRAND {
  const key = channel.toLowerCase() as PlatformKey;
  return PLATFORM_BRAND[key] ?? NEUTRAL_BRAND;
}
