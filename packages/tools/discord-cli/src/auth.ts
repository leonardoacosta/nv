const DISCORD_API_BASE = "https://discord.com/api/v10";

export class DiscordClient {
  private readonly token: string;

  constructor() {
    const token = process.env.DISCORD_BOT_TOKEN;
    if (!token) {
      process.stderr.write(
        "Discord not configured — DISCORD_BOT_TOKEN env var not set\n",
      );
      process.exit(1);
    }
    this.token = token;
  }

  async get(path: string): Promise<unknown> {
    const url = `${DISCORD_API_BASE}${path}`;
    const headers = {
      Authorization: `Bot ${this.token}`,
      "Content-Type": "application/json",
    };

    const response = await fetch(url, { headers });

    if (response.status === 429) {
      const retryAfterHeader = response.headers.get("Retry-After");
      const retryAfterSec = retryAfterHeader
        ? parseFloat(retryAfterHeader)
        : 1;
      await sleep(retryAfterSec * 1000);

      const retryResponse = await fetch(url, { headers });
      if (!retryResponse.ok) {
        await handleHttpError(retryResponse, path);
      }
      return retryResponse.json();
    }

    if (!response.ok) {
      await handleHttpError(response, path);
    }

    return response.json();
  }
}

async function handleHttpError(
  response: Response,
  path: string,
): Promise<never> {
  if (response.status === 401) {
    process.stderr.write(
      "Discord auth failed — check DISCORD_BOT_TOKEN\n",
    );
    process.exit(1);
  }
  if (response.status === 403) {
    const channelMatch = path.match(/^\/channels\/(\d+)/);
    if (channelMatch) {
      process.stderr.write(
        `No permission to read channel ${channelMatch[1]}\n`,
      );
    } else {
      process.stderr.write(`Discord API forbidden: ${path}\n`);
    }
    process.exit(1);
  }
  if (response.status === 404) {
    const guildMatch = path.match(/^\/guilds\/(\d+)/);
    const channelMatch = path.match(/^\/channels\/(\d+)/);
    if (guildMatch) {
      process.stderr.write(`Guild not found: ${guildMatch[1]}\n`);
    } else if (channelMatch) {
      process.stderr.write(`Channel not found: ${channelMatch[1]}\n`);
    } else {
      process.stderr.write(`Discord API not found: ${path}\n`);
    }
    process.exit(1);
  }
  const body = await response.text().catch(() => "");
  process.stderr.write(
    `Discord API error ${response.status}: ${body}\n`,
  );
  process.exit(1);
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
