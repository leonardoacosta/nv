const HELP_TEXT = `Nova Commands

/start     - Start Nova and show status
/help      - Show available commands
/memory    - Read a memory topic (/memory [topic])
/search    - Search messages (/search [query])
/teams     - List recent Teams chats
/calendar  - Today's calendar events
/discord   - List Discord servers
/health    - Fleet service health status
/remind    - Set a reminder (/remind [message] [time])
/ob        - List active obligations
/diary     - Today's interaction summary
/contacts  - List contacts
/soul      - Read Nova's personality
/status    - Daemon and fleet status

Send any other message and Nova will respond via AI.`;

export function buildHelpReply(): string {
  return HELP_TEXT;
}
