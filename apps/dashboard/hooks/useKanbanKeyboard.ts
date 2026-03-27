"use client";

import { useCallback, useEffect } from "react";
import type { DaemonObligation } from "@/types/api";

export interface UseKanbanKeyboardOptions {
  obligations: DaemonObligation[];
  expandedId: string | null;
  onExpand: (id: string) => void;
  onDone: (id: string) => void;
  onDismiss: (id: string) => void;
  onReassign: (id: string) => void;
  active: boolean;
}

/**
 * useKanbanKeyboard — keyboard navigation for the Kanban board.
 *
 * - ArrowUp/ArrowDown: move focus between cards within a lane
 * - ArrowLeft/ArrowRight: move focus between nova/leo columns
 * - Enter: expand/collapse focused card
 * - d: mark focused card done
 * - x: dismiss focused card
 * - r: reassign focused card (toggle owner nova/leo)
 */
export function useKanbanKeyboard({
  obligations,
  expandedId,
  onExpand,
  onDone,
  onDismiss,
  onReassign,
  active,
}: UseKanbanKeyboardOptions) {
  // Collect all card ids in document order for navigation
  const getOrderedIds = useCallback((): string[] => {
    if (typeof document === "undefined") return [];
    const cards = Array.from(
      document.querySelectorAll<HTMLElement>("[id^='kanban-card-']"),
    );
    return cards
      .map((el) => el.id.replace("kanban-card-", ""))
      .filter((id) => obligations.some((o) => o.id === id));
  }, [obligations]);

  const getFocusedId = useCallback((): string | null => {
    if (typeof document === "undefined") return null;
    const focused = document.activeElement?.closest("[id^='kanban-card-']");
    if (!focused) return null;
    return focused.id.replace("kanban-card-", "");
  }, []);

  const focusCard = useCallback((id: string) => {
    const el = document.getElementById(`kanban-card-${id}`);
    if (el) {
      // Focus the first button inside the card
      const btn = el.querySelector<HTMLButtonElement>("button[type='button']");
      btn?.focus();
      el.scrollIntoView({ behavior: "smooth", block: "nearest" });
    }
  }, []);

  const handler = useCallback(
    (e: KeyboardEvent) => {
      if (!active) return;

      // Skip if typing in an input/textarea
      const target = e.target as HTMLElement;
      if (
        target.tagName === "INPUT" ||
        target.tagName === "TEXTAREA" ||
        target.isContentEditable
      ) {
        return;
      }

      const ordered = getOrderedIds();
      const focusedId = getFocusedId() ?? expandedId;

      if (!focusedId && ordered.length > 0) {
        if (e.key === "ArrowDown" || e.key === "ArrowRight") {
          e.preventDefault();
          focusCard(ordered[0]!);
        }
        return;
      }

      if (!focusedId) return;

      const currentIndex = ordered.indexOf(focusedId);

      switch (e.key) {
        case "ArrowDown": {
          e.preventDefault();
          const nextIndex = currentIndex + 1;
          if (nextIndex < ordered.length) {
            focusCard(ordered[nextIndex]!);
          }
          break;
        }
        case "ArrowUp": {
          e.preventDefault();
          const prevIndex = currentIndex - 1;
          if (prevIndex >= 0) {
            focusCard(ordered[prevIndex]!);
          }
          break;
        }
        case "ArrowLeft":
        case "ArrowRight": {
          e.preventDefault();
          const focused = obligations.find((o) => o.id === focusedId);
          if (!focused) break;

          // Find a card in the opposite column
          const targetOwner = focused.owner === "nova" ? "leo" : "nova";
          const targetObligation = obligations.find((o) => o.owner === targetOwner);
          if (targetObligation) {
            focusCard(targetObligation.id);
          }
          break;
        }
        case "Enter": {
          e.preventDefault();
          onExpand(focusedId);
          break;
        }
        case "d": {
          if (e.ctrlKey || e.metaKey || e.altKey) break;
          e.preventDefault();
          onDone(focusedId);
          break;
        }
        case "x": {
          if (e.ctrlKey || e.metaKey || e.altKey) break;
          e.preventDefault();
          onDismiss(focusedId);
          break;
        }
        case "r": {
          if (e.ctrlKey || e.metaKey || e.altKey) break;
          e.preventDefault();
          onReassign(focusedId);
          break;
        }
      }
    },
    [
      active,
      expandedId,
      getOrderedIds,
      getFocusedId,
      focusCard,
      obligations,
      onExpand,
      onDone,
      onDismiss,
      onReassign,
    ],
  );

  useEffect(() => {
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [handler]);
}
