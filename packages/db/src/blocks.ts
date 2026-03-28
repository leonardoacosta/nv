import { z } from "zod";

// ─── Block type schemas ────────────────────────────────────────────────────────

const SectionBlockSchema = z.object({
  type: z.literal("section"),
  title: z.string().optional(),
  data: z.object({
    body: z.string(),
  }),
});

const StatusTableBlockSchema = z.object({
  type: z.literal("status_table"),
  title: z.string().optional(),
  data: z.object({
    columns: z.array(z.string()),
    rows: z.array(z.record(z.string(), z.string())),
  }),
});

const MetricCardBlockSchema = z.object({
  type: z.literal("metric_card"),
  title: z.string().optional(),
  data: z.object({
    label: z.string(),
    value: z.union([z.string(), z.number()]),
    unit: z.string().optional(),
    trend: z.enum(["up", "down", "flat"]).optional(),
    delta: z.string().optional(),
  }),
});

const TimelineBlockSchema = z.object({
  type: z.literal("timeline"),
  title: z.string().optional(),
  data: z.object({
    events: z.array(
      z.object({
        time: z.string(),
        label: z.string(),
        detail: z.string().optional(),
        severity: z.enum(["info", "warning", "error"]).optional(),
      }),
    ),
  }),
});

const ActionGroupBlockSchema = z.object({
  type: z.literal("action_group"),
  title: z.string().optional(),
  data: z.object({
    actions: z.array(
      z.object({
        label: z.string(),
        url: z.string().optional(),
        status: z.enum(["pending", "completed", "dismissed"]).optional(),
      }),
    ),
  }),
});

const KVListBlockSchema = z.object({
  type: z.literal("kv_list"),
  title: z.string().optional(),
  data: z.object({
    items: z.array(
      z.object({
        key: z.string(),
        value: z.string(),
      }),
    ),
  }),
});

const AlertBlockSchema = z.object({
  type: z.literal("alert"),
  title: z.string().optional(),
  data: z.object({
    severity: z.enum(["info", "warning", "error"]),
    message: z.string(),
  }),
});

const SourcePillsBlockSchema = z.object({
  type: z.literal("source_pills"),
  title: z.string().optional(),
  data: z.object({
    sources: z.array(
      z.object({
        name: z.string(),
        status: z.enum(["ok", "unavailable", "empty"]),
      }),
    ),
  }),
});

const PRListBlockSchema = z.object({
  type: z.literal("pr_list"),
  title: z.string().optional(),
  data: z.object({
    prs: z.array(
      z.object({
        title: z.string(),
        repo: z.string(),
        url: z.string().optional(),
        status: z.enum(["open", "merged", "closed"]),
      }),
    ),
  }),
});

const PipelineTableBlockSchema = z.object({
  type: z.literal("pipeline_table"),
  title: z.string().optional(),
  data: z.object({
    pipelines: z.array(
      z.object({
        name: z.string(),
        status: z.enum(["success", "failed", "running", "pending"]),
        duration: z.string().optional(),
      }),
    ),
  }),
});

// ─── Discriminated union ───────────────────────────────────────────────────────

export const BriefingBlockSchema = z.discriminatedUnion("type", [
  SectionBlockSchema,
  StatusTableBlockSchema,
  MetricCardBlockSchema,
  TimelineBlockSchema,
  ActionGroupBlockSchema,
  KVListBlockSchema,
  AlertBlockSchema,
  SourcePillsBlockSchema,
  PRListBlockSchema,
  PipelineTableBlockSchema,
]);

export const BriefingBlocksSchema = z.array(BriefingBlockSchema);

// ─── Inferred types ────────────────────────────────────────────────────────────

export type BriefingBlock = z.infer<typeof BriefingBlockSchema>;
export type BriefingBlocks = z.infer<typeof BriefingBlocksSchema>;
