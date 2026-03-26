# Stage 1: Build
FROM node:22-alpine AS builder

WORKDIR /app

COPY package.json package-lock.json* ./
RUN npm ci

COPY . .
RUN npm run build

# Compile custom server alongside Next.js build
RUN npx tsc --outDir /app/server-dist --module commonjs --target es2017 \
    --esModuleInterop true --skipLibCheck true --noEmit false server.ts

# Stage 2: Runtime
FROM node:22-alpine AS runner

WORKDIR /app

ENV NODE_ENV=production

COPY --from=builder /app/.next/standalone ./
COPY --from=builder /app/.next/static ./.next/static
COPY --from=builder /app/public ./public

# Replace the default standalone server.js with the custom WS-proxy server
COPY --from=builder /app/server-dist/server.js ./server.js

# Ensure http-proxy is available (standalone node_modules may not include it)
COPY --from=builder /app/node_modules/http-proxy ./node_modules/http-proxy
COPY --from=builder /app/node_modules/eventemitter3 ./node_modules/eventemitter3
COPY --from=builder /app/node_modules/follow-redirects ./node_modules/follow-redirects
COPY --from=builder /app/node_modules/requires-port ./node_modules/requires-port

EXPOSE 3000

ENV PORT=3000
ENV HOSTNAME="0.0.0.0"

CMD ["node", "server.js"]
