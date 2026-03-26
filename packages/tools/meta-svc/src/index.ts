import { startServer } from "./server.js";

const PORT = Number(process.env["META_SVC_PORT"] ?? 4008);
await startServer(PORT);
