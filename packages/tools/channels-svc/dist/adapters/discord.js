export class DiscordAdapter {
    name = "discord";
    direction = "bidirectional";
    status() {
        return "disconnected";
    }
    async send(_target, _message) {
        throw new Error("Discord adapter not yet implemented");
    }
}
//# sourceMappingURL=discord.js.map