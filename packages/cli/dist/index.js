#!/usr/bin/env node
/** nv CLI — Nova fleet health, status, and management. */
import { health } from "./commands/health.js";
import { status } from "./commands/status.js";
import { checkCmd } from "./commands/check.js";
import { logs } from "./commands/logs.js";
import { restart } from "./commands/restart.js";
import { pim } from "./commands/pim.js";
import { dreamCmd } from "./commands/dream.js";
import { help } from "./commands/help.js";
const command = process.argv[2] || "health";
switch (command) {
    case "health":
    case "fleet":
        await health();
        break;
    case "status":
        await status();
        break;
    case "check":
        await checkCmd();
        break;
    case "logs":
        logs(process.argv[3]);
        break;
    case "restart":
        await restart(process.argv[3]);
        break;
    case "pim":
        await pim(process.argv[3]);
        break;
    case "dream":
        await dreamCmd(process.argv[3], process.argv.includes("--dry-run"));
        break;
    case "help":
    case "--help":
    case "-h":
        help();
        break;
    default:
        console.error(`Unknown command: ${command}`);
        help();
        process.exit(1);
}
//# sourceMappingURL=index.js.map