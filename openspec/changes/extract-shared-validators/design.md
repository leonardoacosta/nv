# Design: Extract Shared Validators

## Architecture

The validators package sits between `@nova/db` (schema source of truth) and all consumers (tRPC, route handlers, tool services, CLI). It imports Drizzle table objects from `@nova/db` and produces Zod schemas via `drizzle-zod`.

```
@nova/db (Drizzle tables)
    |
    v
@nova/validators (drizzle-zod + business DTOs)
    |
    +---> tRPC procedures (add-trpc-api)
    +---> Dashboard route handlers
    +---> Tool services (MCP inputSchema)
    +---> CLI input validation
```

## Package Structure

```
packages/validators/
  package.json           # @nova/validators, deps: zod, drizzle-zod, peer: @nova/db
  tsconfig.json          # Matches existing packages (ESM, strict, bundler resolution)
  src/
    index.ts             # Barrel export
    common.ts            # Shared schemas: pagination, sorting, date-range, uuid-param
    messages.ts          # insert/select + create/update/filter DTOs
    obligations.ts       # insert/select + create/update/filter DTOs + status enum
    contacts.ts          # insert/select + create/update DTOs
    projects.ts          # insert/select + create/update DTOs + category/status enums (migrated)
    memory.ts            # insert/select + create/update DTOs
    reminders.ts         # insert/select + create/update DTOs
    schedules.ts         # insert/select + create/update DTOs
    sessions.ts          # insert/select + create DTO (read-heavy, minimal write)
    session-events.ts    # insert/select only (append-only table)
    briefings.ts         # insert/select + create DTO
    diary.ts             # insert/select only (write from daemon, read-only in dashboard)
    settings.ts          # insert/select + upsert DTO
```

## Schema Layering Pattern

Each entity file follows a consistent three-layer pattern:

```typescript
// Layer 1: Drizzle-derived base schemas (from drizzle-zod)
import { createInsertSchema, createSelectSchema } from "drizzle-zod";
import { obligations } from "@nova/db";

export const insertObligationSchema = createInsertSchema(obligations);
export const selectObligationSchema = createSelectSchema(obligations);

// Layer 2: Business-logic enums and refinements
export const obligationStatusEnum = z.enum(["open", "in_progress", "done", "cancelled"]);

// Layer 3: DTO schemas (compose from Layer 1 + Layer 2)
export const createObligationSchema = insertObligationSchema
  .omit({ id: true, createdAt: true, updatedAt: true, attemptCount: true, lastAttemptAt: true })
  .extend({
    status: obligationStatusEnum.default("open"),
    priority: z.number().int().min(0).max(4).default(2),
    owner: z.string().default("nova"),
    sourceChannel: z.string().default("dashboard"),
  });

export const updateObligationSchema = createObligationSchema
  .partial()
  .omit({ detectedAction: true, sourceChannel: true });

export const obligationFilterSchema = z.object({
  status: obligationStatusEnum.optional(),
  owner: z.string().optional(),
});

// Type inference
export type CreateObligationInput = z.infer<typeof createObligationSchema>;
export type UpdateObligationInput = z.infer<typeof updateObligationSchema>;
export type ObligationFilter = z.infer<typeof obligationFilterSchema>;
```

## Trade-offs

| Decision | Alternative | Rationale |
|----------|-------------|-----------|
| Separate `@nova/validators` package | Co-locate in `@nova/db` | Keeps db package focused on schema/migrations; validators may have consumers that don't need db client |
| `drizzle-zod` for base schemas | Hand-write all Zod schemas | Eliminates drift risk; Drizzle table is single source of truth |
| Entity-per-file structure | Single flat file | Mirrors db schema structure; enables tree-shaking; easier navigation |
| DTOs compose from insert schemas | DTOs from scratch | Reduces maintenance; changes to Drizzle columns propagate automatically |
| Migrate projects.ts Zod to validators | Leave in db, re-export | Consistent ownership; db package should not depend on Zod |

## Custom Type Handling

The `memory` and `messages` tables use a custom `vector` type. `drizzle-zod` will generate `z.unknown()` for custom types. Override with:

```typescript
export const insertMessageSchema = createInsertSchema(messages, {
  embedding: z.array(z.number()).optional(),
  metadata: z.record(z.unknown()).nullable().optional(),
});
```

The `jsonb` columns (`metadata`, `channelIds`, `toolsUsed`, `sourcesStatus`, `suggestedActions`) similarly need explicit Zod overrides since `drizzle-zod` maps them to `z.unknown()`.
