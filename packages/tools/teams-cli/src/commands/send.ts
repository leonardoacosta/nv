import { MsGraphClient } from "../auth.js";

export async function sendMessage(chatId: string, message: string): Promise<void> {
  const client = new MsGraphClient();

  await client.post(`/chats/${chatId}/messages`, {
    body: {
      content: message,
    },
  });

  process.stdout.write("Sent.\n");
}
