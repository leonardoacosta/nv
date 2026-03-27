"use client";

import { useEffect } from "react";

interface UseApprovalKeyboardOptions {
  /** IDs of pending items in display order. */
  pendingIds: string[];
  /** Currently focused item ID. */
  selectedId: string | null;
  /** Callback to change the focused item. */
  onNavigate: (id: string) => void;
  /** Approve the currently selected item. */
  onApprove: (id: string) => void;
  /** Dismiss the currently selected item. */
  onDismiss: (id: string) => void;
  /** Whether an action is in-flight (disables shortcuts). */
  busy: boolean;
}

/**
 * Registers keyboard shortcuts for the Approvals page:
 *   A        = approve selected
 *   D        = dismiss selected
 *   J / Down = next item
 *   K / Up   = previous item
 *   Enter    = confirm/select (same as approve)
 *
 * Shortcuts are suppressed when the active element is an input, textarea, or
 * select to avoid conflicts with form fields.
 */
export function useApprovalKeyboard({
  pendingIds,
  selectedId,
  onNavigate,
  onApprove,
  onDismiss,
  busy,
}: UseApprovalKeyboardOptions): void {
  useEffect(() => {
    function handler(e: KeyboardEvent) {
      // Don't intercept when user is typing in a form field
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;

      if (busy || pendingIds.length === 0) return;

      const currentIndex = selectedId
        ? pendingIds.indexOf(selectedId)
        : -1;

      switch (e.key) {
        case "j":
        case "ArrowDown": {
          e.preventDefault();
          const next = Math.min(currentIndex + 1, pendingIds.length - 1);
          const nextId = pendingIds[next];
          if (nextId) onNavigate(nextId);
          break;
        }
        case "k":
        case "ArrowUp": {
          e.preventDefault();
          const prev = Math.max(currentIndex - 1, 0);
          const prevId = pendingIds[prev];
          if (prevId) onNavigate(prevId);
          break;
        }
        case "a":
        case "A": {
          if (selectedId) {
            e.preventDefault();
            onApprove(selectedId);
          }
          break;
        }
        case "d":
        case "D": {
          if (selectedId) {
            e.preventDefault();
            onDismiss(selectedId);
          }
          break;
        }
        case "Enter": {
          if (selectedId) {
            e.preventDefault();
            onApprove(selectedId);
          }
          break;
        }
      }
    }

    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [pendingIds, selectedId, onNavigate, onApprove, onDismiss, busy]);
}
