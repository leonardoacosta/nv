import { z } from "zod/v4";

// Pagination: offset-based
export const paginationSchema = z.object({
  limit: z.number().int().min(1).max(100).default(20),
  offset: z.number().int().min(0).default(0),
});
export type PaginationInput = z.infer<typeof paginationSchema>;

// Pagination: cursor-based
export const cursorPaginationSchema = z.object({
  cursor: z.string().uuid().optional(),
  limit: z.number().int().min(1).max(100).default(20),
});
export type CursorPaginationInput = z.infer<typeof cursorPaginationSchema>;

// Sort order
export const sortOrderSchema = z.enum(["asc", "desc"]).default("desc");
export type SortOrder = z.infer<typeof sortOrderSchema>;

// Date range filter
export const dateRangeSchema = z.object({
  from: z.coerce.date().optional(),
  to: z.coerce.date().optional(),
});
export type DateRangeInput = z.infer<typeof dateRangeSchema>;

// UUID parameter (for route params)
export const uuidParamSchema = z.object({
  id: z.string().uuid(),
});
export type UuidParam = z.infer<typeof uuidParamSchema>;
