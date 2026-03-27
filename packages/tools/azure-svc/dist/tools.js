export class ToolRegistry {
    tools = new Map();
    register(tool) {
        this.tools.set(tool.name, tool);
    }
    get(name) {
        return this.tools.get(name);
    }
    list() {
        return Array.from(this.tools.values());
    }
    async execute(name, input) {
        const tool = this.tools.get(name);
        if (!tool) {
            throw new Error(`Unknown tool: ${name}`);
        }
        return tool.handler(input);
    }
}
export const pingTool = {
    name: "ping",
    description: "Returns pong to verify the service is running",
    inputSchema: { type: "object", properties: {}, additionalProperties: false },
    handler: async () => "pong",
};
//# sourceMappingURL=tools.js.map