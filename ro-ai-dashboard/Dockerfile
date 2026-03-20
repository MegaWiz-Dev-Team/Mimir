# ══════════════════════════════════════
# 🧠 Mimir Dashboard — Next.js Multi-Stage Build
# ══════════════════════════════════════
# Build: docker build -t mimir-dashboard .
# Run:   docker run -p 3001:3000 mimir-dashboard

# ── Stage 1: Dependencies ──
FROM node:22-alpine AS deps
WORKDIR /app

COPY package.json package-lock.json ./
RUN npm ci --ignore-scripts

# ── Stage 2: Build ──
FROM node:22-alpine AS builder
WORKDIR /app

COPY --from=deps /app/node_modules ./node_modules
COPY . .

ENV NEXT_TELEMETRY_DISABLED=1

# Allow overriding OIDC client_id at build time (empty = dev mode, skip SSO)
ARG NEXT_PUBLIC_YGGDRASIL_CLIENT_ID
ENV NEXT_PUBLIC_YGGDRASIL_CLIENT_ID=${NEXT_PUBLIC_YGGDRASIL_CLIENT_ID}

RUN npm run build

# ── Stage 3: Runtime ──
FROM node:22-alpine AS runner
WORKDIR /app

ENV NODE_ENV=production
ENV NEXT_TELEMETRY_DISABLED=1

# Run as non-root
RUN addgroup --system --gid 1001 nodejs \
    && adduser --system --uid 1001 nextjs

# Copy build output
COPY --from=builder /app/public ./public
COPY --from=builder --chown=nextjs:nodejs /app/.next/standalone ./
COPY --from=builder --chown=nextjs:nodejs /app/.next/static ./.next/static

USER nextjs

EXPOSE 3000
ENV PORT=3000
ENV HOSTNAME="0.0.0.0"

CMD ["node", "server.js"]
