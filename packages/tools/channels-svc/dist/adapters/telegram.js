export class TelegramAdapter {
    name = "telegram";
    direction = "bidirectional";
    botToken;
    constructor(botToken) {
        this.botToken = botToken;
    }
    status() {
        return this.botToken ? "connected" : "disconnected";
    }
    async send(chatId, message) {
        if (!this.botToken) {
            throw new Error("Telegram bot token not configured");
        }
        const url = `https://api.telegram.org/bot${this.botToken}/sendMessage`;
        const response = await fetch(url, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({
                chat_id: chatId,
                text: message,
            }),
        });
        if (!response.ok) {
            const body = await response.text();
            throw new Error(`Telegram API error (${response.status}): ${body}`);
        }
    }
}
//# sourceMappingURL=telegram.js.map