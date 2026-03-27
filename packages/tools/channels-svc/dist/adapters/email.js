export class EmailAdapter {
    name = "email";
    direction = "outbound";
    status() {
        return "disconnected";
    }
    async send(_target, _message) {
        throw new Error("Email adapter not yet implemented");
    }
}
//# sourceMappingURL=email.js.map