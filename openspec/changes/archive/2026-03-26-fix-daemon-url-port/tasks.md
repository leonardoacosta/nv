# Implementation Tasks

<!-- beads:epic:nv-jghw -->

## Config Batch

- [ ] [1.1] [P-0] `docker-compose.yml`: change `DAEMON_URL=http://host.docker.internal:3443` to `DAEMON_URL=http://host.docker.internal:8400` [owner:devops-engineer]
- [ ] [1.2] [P-0] `apps/dashboard/lib/daemon.ts`: change default fallback from `"http://127.0.0.1:3443"` to `"http://127.0.0.1:8400"` [owner:ui-engineer]
- [ ] [1.3] [P-0] `~/dev/hl/homelab/traefik/dynamic/routes.yml`: change `nova-daemon` loadBalancer server URL from `http://172.20.0.1:3443` to `http://172.20.0.1:8400` [owner:devops-engineer]
- [ ] [1.4] [P-0] Restart dashboard container after compose change: `docker compose down && docker compose up -d` to apply new `DAEMON_URL` env var [owner:devops-engineer]
