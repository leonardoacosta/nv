import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

/**
 * Merge Tailwind CSS classes with clsx and tailwind-merge.
 *
 * Combines conditional class logic (clsx) with intelligent Tailwind
 * class deduplication (tailwind-merge) to prevent conflicting utilities.
 */
export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}
