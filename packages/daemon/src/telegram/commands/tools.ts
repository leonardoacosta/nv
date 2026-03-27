import type TelegramBot from "node-telegram-bot-api";

/**
 * /tools — inline keyboard focused on work tools
 */
export function buildToolsKeyboard(): TelegramBot.InlineKeyboardMarkup {
  return {
    inline_keyboard: [
      [
        { text: "Teams Chats", callback_data: "cmd:teams" },
        { text: "Calendar", callback_data: "cmd:calendar" },
        { text: "Mail Inbox", callback_data: "cmd:mail" },
      ],
      [
        { text: "ADO Work Items", callback_data: "cmd:ado wi" },
        { text: "ADO PRs", callback_data: "cmd:ado prs" },
        { text: "ADO Builds", callback_data: "cmd:ado" },
      ],
      [
        { text: "PIM Status", callback_data: "cmd:pim" },
        { text: "PIM Activate All", callback_data: "cmd:pim all" },
        { text: "Azure CLI", callback_data: "cmd:az" },
      ],
      [
        { text: "Discord", callback_data: "cmd:discord" },
        { text: "Contacts", callback_data: "cmd:contacts" },
        { text: "Soul", callback_data: "cmd:soul" },
      ],
    ],
  };
}
