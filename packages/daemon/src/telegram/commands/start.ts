import type TelegramBot from "node-telegram-bot-api";

/**
 * /start — inline keyboard dashboard for quick navigation
 */
export function buildStartKeyboard(): TelegramBot.InlineKeyboardMarkup {
  return {
    inline_keyboard: [
      [
        { text: "Snapshot", callback_data: "cmd:snapshot" },
        { text: "Calendar", callback_data: "cmd:calendar" },
        { text: "Mail", callback_data: "cmd:mail" },
      ],
      [
        { text: "Obligations", callback_data: "cmd:ob" },
        { text: "Memory", callback_data: "cmd:memory" },
        { text: "Health", callback_data: "cmd:health" },
      ],
      [
        { text: "Teams", callback_data: "cmd:teams" },
        { text: "ADO", callback_data: "cmd:ado" },
        { text: "PIM", callback_data: "cmd:pim" },
      ],
      [
        { text: "Dream", callback_data: "cmd:dream" },
        { text: "Azure", callback_data: "cmd:az" },
        { text: "Discord", callback_data: "cmd:discord" },
      ],
    ],
  };
}
