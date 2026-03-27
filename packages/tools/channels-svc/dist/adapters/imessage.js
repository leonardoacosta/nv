export class IMessageAdapter {
    name = "imessage";
    direction = "bidirectional";
    status() {
        return "disconnected";
    }
    async send(_target, _message) {
        throw new Error("iMessage adapter not yet implemented");
    }
}
//# sourceMappingURL=imessage.js.map