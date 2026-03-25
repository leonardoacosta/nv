FROM node:20-slim

# Install Claude Code globally
RUN npm install -g @anthropic-ai/claude-code

# Create a non-root sandbox user with home directory
RUN useradd -m -d /home/sandbox -s /bin/bash sandbox

# Set sandbox home as working directory
WORKDIR /home/sandbox

# Switch to sandbox user for runtime
USER sandbox

# Expose labels for identification
LABEL org.opencontainers.image.title="nova-cc-session"
LABEL org.opencontainers.image.description="Claude Code session container for Nova dashboard"

# stream-json protocol: read from stdin, write to stdout
ENTRYPOINT ["claude", "--input-format", "stream-json", "--output-format", "stream-json", "--dangerously-skip-permissions", "--no-session-persistence"]
