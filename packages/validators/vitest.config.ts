import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    env: {
      // Dummy value: @nova/db barrel re-exports the client which requires
      // DATABASE_URL at module load time. The validators only use table
      // definitions (for drizzle-zod), never the actual db connection.
      DATABASE_URL: "postgresql://unused:unused@localhost:5432/unused",
    },
  },
});
