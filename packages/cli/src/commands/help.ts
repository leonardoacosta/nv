/** nv help — usage information. */

import { bold, gray } from "../lib/format.js";

export function help(): void {
  console.log(`
${bold("nv")} - Nova CLI

${bold("USAGE")}
  nv <command> [args]

${bold("COMMANDS")}
  health          Fleet + daemon + dashboard health ${gray("(default)")}
  status          Quick one-line status
  check           Full connectivity check (fleet, postgres, SSH, Doppler)
  fleet           Alias for health (fleet focus)
  logs [service]  Tail service logs ${gray("(daemon, fleet, memory, ...)")}
  restart [svc]   Restart service ${gray("(daemon, fleet, all, ...)")}
  pim [action]    PIM activation ${gray("(status, all, <number>)")}
  help            Show this help

${bold("LOG/RESTART TARGETS")}
  daemon, fleet, router, memory, messages, channels,
  discord, teams, schedule, graph, meta, azure, all

${bold("EXAMPLES")}
  nv                    Show fleet health
  nv status             Quick status line
  nv check              Full connectivity check
  nv logs memory        Tail memory-svc logs
  nv restart fleet      Restart all fleet services
  nv pim 3              Activate PIM role 3
  nv pim all            Activate all PIM roles
`);
}
