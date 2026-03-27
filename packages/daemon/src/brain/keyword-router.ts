/**
 * Tier 1: Regex/keyword-based message router.
 * Tests incoming text against a static pattern table and returns the first match.
 * Deterministic matching yields high confidence (0.95).
 */

export interface KeywordMatch {
  tool: string;
  port: number;
  params: Record<string, unknown>;
  confidence: number;
}

interface PatternEntry {
  regex: RegExp;
  tool: string;
  port: number;
  extractParams?: (match: RegExpMatchArray, text: string) => Record<string, unknown>;
}

const PATTERNS: PatternEntry[] = [
  // ── Calendar ────────────────────────────────────────────────────────────────
  {
    regex: /(?:what(?:'s| is) on my (?:calendar|schedule)|today(?:'s)? (?:schedule|agenda|events|meetings)|my agenda|do i have (?:any )?meetings today|am i free today)/,
    tool: "calendar_today",
    port: 4106,
  },
  {
    regex: /(?:upcoming (?:events|meetings|appointments)|what(?:'s| is) (?:next|coming up)|next meeting|next event|what do i have (?:this week|tomorrow|coming up))/,
    tool: "calendar_upcoming",
    port: 4106,
  },

  // ── Email (send before inbox — more specific patterns first) ────────────────
  {
    regex: /(?:send (?:an? )?email|email .+ about|compose (?:an? )?(?:email|mail)|write (?:an? )?(?:email|mail))/,
    tool: "email_send",
    port: 4103,
    extractParams: (_match, text) => ({ rawText: text }),
  },
  {
    regex: /(?:check (?:my )?(?:email|inbox|mail)|any (?:new )?(?:emails?|mail)|(?:new |unread |recent )?(?:emails?|inbox)|show (?:my )?(?:inbox|email|mail))/,
    tool: "email_inbox",
    port: 4103,
  },

  // ── Weather ─────────────────────────────────────────────────────────────────
  {
    regex: /(?:weather|forecast|temperature|how(?:'s| is) (?:it |the weather )?outside|is it (?:going to )?rain)/,
    tool: "weather_current",
    port: 4104,
  },

  // ── Reminders ───────────────────────────────────────────────────────────────
  {
    regex: /(?:my reminders|(?:list|show|what are) (?:my )?reminders|what do i need to do|pending (?:tasks|reminders)|to-?do list)/,
    tool: "reminders_list",
    port: 4106,
  },
  {
    regex: /(?:remind me (?:to|about|that)|set (?:a )?reminder|create (?:a )?reminder)/,
    tool: "reminder_create",
    port: 4106,
    extractParams: (_match, text) => ({ rawText: text }),
  },

  // ── Memory ──────────────────────────────────────────────────────────────────
  {
    regex: /(?:what do you (?:know|remember) about|recall .+|do you remember|what(?:'s| is) in (?:your )?memory about)/,
    tool: "memory_read",
    port: 4101,
    extractParams: (_match, text) => {
      const topicMatch = text.match(/(?:about|recall)\s+(.+)/i);
      return topicMatch ? { topic: topicMatch[1].trim() } : {};
    },
  },

  // ── Messages ────────────────────────────────────────────────────────────────
  {
    regex: /(?:recent messages|latest messages|show (?:me )?(?:recent |latest )?messages|message history|what(?:'s| has) been said)/,
    tool: "messages_recent",
    port: 4102,
  },

  // ── Contacts ────────────────────────────────────────────────────────────────
  {
    regex: /(?:who is .+|contact (?:info|details|information) for|look ?up .+ contact|find .+ (?:number|email|contact))/,
    tool: "contact_lookup",
    port: 4105,
    extractParams: (_match, text) => {
      const nameMatch = text.match(/(?:who is|contact (?:info|details|information) for|look ?up|find)\s+(.+?)(?:\s+(?:contact|number|email))?$/i);
      return nameMatch ? { name: nameMatch[1].trim() } : {};
    },
  },

  // ── System Health ───────────────────────────────────────────────────────────
  {
    regex: /(?:system (?:status|health)|health ?check|are (?:services?|things?|systems?) (?:running|up|ok|working)|service status|fleet (?:status|health))/,
    tool: "health_check",
    port: 4100,
  },

  // ── Date/Time ───────────────────────────────────────────────────────────────
  {
    regex: /(?:what time (?:is it|now)|current (?:time|date)|what(?:'s| is) (?:the )?(?:date|time)|today(?:'s)? date)/,
    tool: "datetime_now",
    port: 4108,
  },

  // ── Teams ───────────────────────────────────────────────────────────────────
  {
    regex: /(?:teams? (?:messages?|chats?|updates?)|check teams|any teams? (?:messages?|updates?)|what(?:'s| is) (?:new |happening )?(?:on|in) teams)/,
    tool: "teams_recent",
    port: 4107,
  },

  // ── Azure DevOps ────────────────────────────────────────────────────────────
  {
    regex: /(?:ado (?:status|tasks?|work items?|updates?)|devops (?:status|tasks?)|my (?:work items?|pull requests?|prs?)|check (?:ado|devops))/,
    tool: "ado_status",
    port: 4109,
  },

  // ── Discord ─────────────────────────────────────────────────────────────────
  {
    regex: /(?:discord (?:messages?|updates?|servers?)|check discord|any discord (?:messages?|updates?)|what(?:'s| is) (?:new |happening )?(?:on|in) discord)/,
    tool: "discord_recent",
    port: 4102,
  },

  // ── Diary ───────────────────────────────────────────────────────────────────
  {
    regex: /(?:(?:show|read|get) (?:my )?diary|diary (?:entries?|summary|today)|what happened (?:today|yesterday)|daily (?:summary|recap))/,
    tool: "diary_read",
    port: 4101,
  },

  // ── Obligations ─────────────────────────────────────────────────────────────
  {
    regex: /(?:my obligations|(?:list|show|what are) (?:my )?obligations|outstanding commitments|what (?:did i|have i) (?:promised|committed))/,
    tool: "obligations_list",
    port: 4101,
  },
];

export class KeywordRouter {
  /**
   * Test the input text against all patterns, returning the first match.
   * Returns null if no pattern matches (falls through to next tier).
   */
  match(text: string): KeywordMatch | null {
    const lower = text.toLowerCase().trim();

    for (const entry of PATTERNS) {
      const m = lower.match(entry.regex);
      if (m) {
        const params = entry.extractParams
          ? entry.extractParams(m, text)
          : {};
        return {
          tool: entry.tool,
          port: entry.port,
          params,
          confidence: 0.95,
        };
      }
    }

    return null;
  }
}
