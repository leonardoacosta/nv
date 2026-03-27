# Stage 1: Build
FROM node:22-alpine AS builder

RUN corepack enable && corepack prepare pnpm@latest --activate

WORKDIR /app

# Copy workspace root config
COPY pnpm-workspace.yaml package.json pnpm-lock.yaml turbo.json ./

# Copy all workspace package.json files needed for dashboard
COPY packages/db/package.json packages/db/
COPY packages/api/package.json packages/api/
COPY packages/auth/package.json packages/auth/
COPY packages/ui/package.json packages/ui/
COPY packages/validators/package.json packages/validators/
COPY apps/dashboard/package.json apps/dashboard/

# Install dependencies
RUN pnpm install --frozen-lockfile

# Copy all workspace package sources
COPY packages/db/ packages/db/
COPY packages/api/ packages/api/
COPY packages/auth/ packages/auth/
COPY packages/ui/ packages/ui/
COPY packages/validators/ packages/validators/
COPY apps/dashboard/ apps/dashboard/

# Build workspace deps first (db, api, auth — validators is not in the dep graph).
RUN pnpm --filter @nova/db build && \
    pnpm --filter @nova/api build && \
    pnpm --filter @nova/auth build

# Build dashboard. Dummy env vars prevent runtime-init checks from throwing
# during next build page-data collection — they are NOT used for actual queries.
ENV DATABASE_URL=postgresql://build:build@localhost:5432/build
ENV BETTER_AUTH_SECRET=build-time-placeholder
RUN pnpm --filter nova-dashboard build
# Clear build-only env vars so they don't leak to later stages
ENV DATABASE_URL=
ENV BETTER_AUTH_SECRET=

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
