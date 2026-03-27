import { z } from "zod/v4";
export declare const paginationSchema: z.ZodObject<{
    limit: z.ZodDefault<z.ZodNumber>;
    offset: z.ZodDefault<z.ZodNumber>;
}, z.core.$strip>;
export type PaginationInput = z.infer<typeof paginationSchema>;
export declare const cursorPaginationSchema: z.ZodObject<{
    cursor: z.ZodOptional<z.ZodString>;
    limit: z.ZodDefault<z.ZodNumber>;
}, z.core.$strip>;
export type CursorPaginationInput = z.infer<typeof cursorPaginationSchema>;
export declare const sortOrderSchema: z.ZodDefault<z.ZodEnum<{
    asc: "asc";
    desc: "desc";
}>>;
export type SortOrder = z.infer<typeof sortOrderSchema>;
export declare const dateRangeSchema: z.ZodObject<{
    from: z.ZodOptional<z.ZodCoercedDate<unknown>>;
    to: z.ZodOptional<z.ZodCoercedDate<unknown>>;
}, z.core.$strip>;
export type DateRangeInput = z.infer<typeof dateRangeSchema>;
export declare const uuidParamSchema: z.ZodObject<{
    id: z.ZodString;
}, z.core.$strip>;
export type UuidParam = z.infer<typeof uuidParamSchema>;
//# sourceMappingURL=common.d.ts.map