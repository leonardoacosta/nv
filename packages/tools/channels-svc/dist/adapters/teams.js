export class TeamsAdapter {
    name = "teams";
    direction = "bidirectional";
    status() {
        return "disconnected";
    }
    async send(_target, _message) {
        throw new Error("Teams adapter not yet implemented");
    }
}
//# sourceMappingURL=teams.js.map