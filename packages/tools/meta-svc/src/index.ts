import { startServer } from "./server.js";

const PORT = Number(process.env["META_SVC_PORT"] ?? 4108);
await startServer(PORT);
