import { z } from "zod";

/** Validates HH:MM time format */
const timeSchema = z
  .string()
  .regex(/^\d{2}:\d{2}$/, "Must be in HH:MM format");

/** Validates a number is in range 0-23 (hours) */
const hourSchema = z.number().int().min(0).max(23);

export const configSchema = z.object({
  daemon: z.object({
    port: z
      .union([z.number(), z.string()])
      .transform((v) => (typeof v === "string" ? parseInt(v, 10) : v))
      .pipe(z.number().int().min(1).max(65535))
      .default(7700),
    logLevel: z
      .enum(["trace", "debug", "info", "warn", "error", "fatal"])
      .default("info"),
    toolRouterUrl: z.string().url().default("http://localhost:4100"),
  }),

  agent: z.object({
    model: z.string().min(1).default("claude-opus-4-6"),
    maxTurns: z
      .union([z.number(), z.string()])
      .transform((v) => (typeof v === "string" ? parseInt(v, 10) : v))
      .pipe(z.number().int().min(1).max(1000))
      .default(100),
    systemPromptPath: z.string().default("config/system-prompt.md"),
  }),

  telegram: z
    .object({
      botToken: z.string().min(1),
      chatId: z.string().optional(),
    })
    .optional(),

  discord: z
    .object({
      botToken: z.string().min(1),
    })
    .optional(),

  teams: z
    .object({
      webhookUrl: z.string().url(),
    })
    .optional(),

  digest: z.object({
    enabled: z.boolean().default(true),
    quietStart: timeSchema.default("22:00"),
    quietEnd: timeSchema.default("07:00"),
    tier1Hours: z.array(hourSchema).default([7, 12, 17]),
    cooldowns: z
      .object({
        p0Ms: z.number().int().positive().default(1_800_000),
        p1Ms: z.number().int().positive().default(14_400_000),
        p2Ms: z.number().int().positive().default(43_200_000),
        hashTtlMs: z.number().int().positive().default(172_800_000),
      })
      .default({}),
  }).default({}),

  autonomy: z
    .object({
      enabled: z.boolean().default(true),
      timeoutMs: z.number().int().positive().default(300_000),
      cooldownHours: z.number().positive().default(2),
      dailyBudgetUsd: z.number().positive().default(5.0),
    })
    .optional(),

  queue: z.object({
    concurrency: z.number().int().min(1).max(32).default(2),
    maxQueueSize: z.number().int().min(1).max(1000).default(20),
  }).default({}),

  database: z.object({
    url: z.string().min(1),
  }),
});

export type ValidatedConfig = z.infer<typeof configSchema>;
