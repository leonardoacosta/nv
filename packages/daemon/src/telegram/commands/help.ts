const HELP_TEXT = `Nova Commands

Navigation:
  /start    Dashboard with quick actions
  /tools    Tool keyboard menu
  /help     This message

Data:
  /brief    Briefing (calendar + mail + obligations)
  /calendar Today's events
  /mail     Outlook email (/mail inbox|read|search)
  /teams    Teams chats
  /discord  Discord servers
  /memory   Read memory (/memory [topic])
  /search   Search messages (/search [query])

Work:
  /ob       Active obligations
  /ado      Azure DevOps (/ado wi|prs|repos)
  /pim      PIM roles (/pim status|all|N)
  /az       Azure CLI (/az [command])
  /remind   Set reminder (/remind msg time)
  /contacts Contact list

System:
  /health   Fleet service health
  /status   Daemon + fleet status
  /dream    Memory consolidation (/dream status)
  /soul     Nova personality
  /diary    Interaction summary (/diary [date])

Send any other message and Nova will respond via AI.`;

export function buildHelpReply(): string {
  return HELP_TEXT;
}
