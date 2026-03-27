# Stage 1: Build
FROM node:22-alpine AS builder

RUN corepack enable && corepack prepare pnpm@latest --activate

WORKDIR /app

# Copy workspace root config
COPY pnpm-workspace.yaml package.json pnpm-lock.yaml ./

# Copy only the packages needed for dashboard
COPY packages/db/package.json packages/db/
COPY apps/dashboard/package.json apps/dashboard/

# Install dependencies
RUN pnpm install --frozen-lockfile

# Copy source for @nova/db and dashboard
COPY packages/db/ packages/db/
COPY apps/dashboard/ apps/dashboard/

# Build @nova/db first (dashboard depends on it)
RUN pnpm --filter @nova/db build

# Build dashboard (dummy DATABASE_URL prevents @nova/db client from throwing
# at import time during next build page-data collection — not used for queries)
RUN DATABASE_URL=postgresql://build:build@localhost:5432/build \
    pnpm --filter nova-dashboard build

# Stage 2: Runtime
FROM node:22-alpine AS runner

WORKDIR /app

ENV NODE_ENV=production

COPY --from=builder /app/apps/dashboard/.next/standalone ./
COPY --from=builder /app/apps/dashboard/.next/static ./apps/dashboard/.next/static
COPY --from=builder /app/apps/dashboard/public ./apps/dashboard/public

EXPOSE 3000

ENV PORT=3000
ENV HOSTNAME="0.0.0.0"

CMD ["node", "apps/dashboard/server.js"]
