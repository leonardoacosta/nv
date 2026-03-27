import { pgTable, text, timestamp, uuid } from "drizzle-orm/pg-core";
import { z } from "zod";

export const projects = pgTable("projects", {
  id: uuid("id").primaryKey().defaultRandom(),
  code: text("code").notNull().unique(),
  name: text("name").notNull(),
  category: text("category").notNull().default("work"),
  status: text("status").notNull().default("active"),
  description: text("description"),
  content: text("content"),
  path: text("path"),
  createdAt: timestamp("created_at", { withTimezone: true }).notNull().defaultNow(),
  updatedAt: timestamp("updated_at", { withTimezone: true }).notNull().defaultNow(),
});

export type Project = typeof projects.$inferSelect;
export type NewProject = typeof projects.$inferInsert;

// Zod validation schemas

export const projectCategoryEnum = z.enum(["work", "personal", "open_source", "archived"]);
export type ProjectCategory = z.infer<typeof projectCategoryEnum>;

export const projectStatusEnum = z.enum(["active", "paused", "completed", "archived"]);
export type ProjectStatus = z.infer<typeof projectStatusEnum>;

export const createProjectSchema = z.object({
  code: z.string().min(1),
  name: z.string().min(1),
  category: projectCategoryEnum.optional(),
  status: projectStatusEnum.optional(),
  path: z.string().optional(),
});
export type CreateProjectInput = z.infer<typeof createProjectSchema>;

export const updateProjectSchema = z.object({
  name: z.string().min(1).optional(),
  category: projectCategoryEnum.optional(),
  status: projectStatusEnum.optional(),
});
export type UpdateProjectInput = z.infer<typeof updateProjectSchema>;
