const DISCORD_API_BASE = "https://discord.com/api/v10";

export class DiscordApiError extends Error {
  constructor(
    message: string,
    public readonly status: number,
  ) {
    super(message);
    this.name = "DiscordApiError";
  }
}

export class DiscordClient {
  private readonly token: string;

  constructor(token: string) {
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
        handleHttpError(retryResponse, path);
      }
      return retryResponse.json();
    }

    if (!response.ok) {
      handleHttpError(response, path);
    }

    return response.json();
  }
}

function handleHttpError(response: Response, path: string): never {
  if (response.status === 401) {
    throw new DiscordApiError("Discord auth failed — check DISCORD_BOT_TOKEN", 401);
  }

  if (response.status === 403) {
    const channelMatch = path.match(/^\/channels\/(\d+)/);
    if (channelMatch) {
      throw new DiscordApiError(
        `No permission to read channel ${channelMatch[1]}`,
        403,
      );
    }
    throw new DiscordApiError(`Discord API forbidden: ${path}`, 403);
  }

  if (response.status === 404) {
    const guildMatch = path.match(/^\/guilds\/(\d+)/);
    const channelMatch = path.match(/^\/channels\/(\d+)/);
    if (guildMatch) {
      throw new DiscordApiError(`Guild not found: ${guildMatch[1]}`, 404);
    }
    if (channelMatch) {
      throw new DiscordApiError(`Channel not found: ${channelMatch[1]}`, 404);
    }
    throw new DiscordApiError(`Discord API not found: ${path}`, 404);
  }

  throw new DiscordApiError(
    `Discord API error ${response.status}: ${path}`,
    response.status,
  );
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
