export class DiscordAdapter {
    name = "discord";
    direction = "bidirectional";
    botToken;
    constructor(botToken) {
        this.botToken = botToken;
    }
    status() {
        return this.botToken ? "connected" : "disconnected";
    }
    async send(channelId, message) {
        if (!this.botToken) {
            throw new Error("Discord bot token not configured");
        }
        const url = `https://discord.com/api/v10/channels/${channelId}/messages`;
        const response = await fetch(url, {
            method: "POST",
            headers: {
                "Authorization": `Bot ${this.botToken}`,
                "Content-Type": "application/json",
            },
            body: JSON.stringify({ content: message }),
        });
        if (!response.ok) {
            const body = await response.text();
            throw new Error(`Discord API error (${response.status}): ${body}`);
        }
    }
}
//# sourceMappingURL=discord.js.map