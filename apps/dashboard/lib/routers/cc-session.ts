/**
 * Dashboard-local tRPC router for CC session management.
 *
 * These procedures wrap the existing sessionManager which depends on
 * Docker APIs. They live in apps/dashboard/ to avoid pulling Docker SDK
 * into the @nova/api package.
 */

import { z } from "zod";
import { TRPCError } from "@trpc/server";
import { createTRPCRouter, protectedProcedure } from "@nova/api";

import { sessionManager } from "@/lib/session-manager";

export const ccSessionRouter = createTRPCRouter({
  ccSession: createTRPCRouter({
    /**
     * Control the CC session (start, stop, restart).
     */
    control: protectedProcedure
      .input(
        z.object({
          action: z.enum(["start", "stop", "restart"]),
        }),
      )
      .mutation(async ({ input }) => {
        try {
          if (input.action === "start") {
            await sessionManager.start();
          } else if (input.action === "stop") {
            await sessionManager.stop();
          } else {
            await sessionManager.restart();
          }

          return { status: sessionManager.getStatus() };
        } catch (err) {
          const message = err instanceof Error ? err.message : String(err);
          throw new TRPCError({
            code: "INTERNAL_SERVER_ERROR",
            message,
          });
        }
      }),

    /**
     * Get CC session logs.
     */
    logs: protectedProcedure
      .input(z.object({ lines: z.number().int().min(1).max(500).default(50) }))
      .query(async ({ input }) => {
        const lines = await sessionManager.getLogs(input.lines);
        return { lines };
      }),

    /**
     * Send a message to the CC session.
     */
    message: protectedProcedure
      .input(
        z.object({
          text: z.string().min(1),
          message_id: z.string().optional(),
          chat_id: z.string().optional(),
          context: z.record(z.string(), z.unknown()).optional(),
        }),
      )
      .mutation(async ({ input }) => {
        const controller = new AbortController();
        const timeoutId = setTimeout(() => controller.abort(), 120_000);

        try {
          const sendPromise = sessionManager.sendMessage(
            input.text,
            input.context,
          );

          const result = await Promise.race<
            Awaited<ReturnType<typeof sessionManager.sendMessage>> | never
          >([
            sendPromise,
            new Promise<never>((_, reject) => {
              controller.signal.addEventListener("abort", () =>
                reject(new Error("Request timed out")),
              );
            }),
          ]);

          const status = sessionManager.getStatus();

          return {
            reply: result.reply,
            session_state: status.state,
            processing_ms: result.processing_ms,
          };
        } catch (err) {
          const message = err instanceof Error ? err.message : String(err);

          if (message.includes("timed out")) {
            throw new TRPCError({
              code: "TIMEOUT",
              message: "Request timed out",
            });
          }

          if (message.includes("not ready")) {
            throw new TRPCError({
              code: "PRECONDITION_FAILED",
              message,
            });
          }

          throw new TRPCError({
            code: "INTERNAL_SERVER_ERROR",
            message,
          });
        } finally {
          clearTimeout(timeoutId);
        }
      }),

    /**
     * Get CC session status.
     */
    status: protectedProcedure.query(() => {
      return sessionManager.getStatus();
    }),
  }),
});
