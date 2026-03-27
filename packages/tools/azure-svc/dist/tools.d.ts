export interface ToolDefinition {
    name: string;
    description: string;
    inputSchema: Record<string, unknown>;
    handler: (input: Record<string, unknown>) => Promise<string>;
}
export declare class ToolRegistry {
    private tools;
    register(tool: ToolDefinition): void;
    get(name: string): ToolDefinition | undefined;
    list(): ToolDefinition[];
    execute(name: string, input: Record<string, unknown>): Promise<string>;
}
export declare const pingTool: ToolDefinition;
