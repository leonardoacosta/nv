export class AdapterRegistry {
    adapters = new Map();
    register(adapter) {
        this.adapters.set(adapter.name, adapter);
    }
    get(name) {
        return this.adapters.get(name);
    }
    list() {
        return Array.from(this.adapters.values()).map((adapter) => ({
            name: adapter.name,
            status: adapter.status(),
            direction: adapter.direction,
        }));
    }
    has(name) {
        return this.adapters.has(name);
    }
}
//# sourceMappingURL=registry.js.map