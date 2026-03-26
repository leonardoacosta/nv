import { MsGraphClient } from "../auth.js";

interface Presence {
  id: string;
  availability: string;
  activity: string;
  statusMessage?: {
    message?: { content?: string };
  } | null;
}

export async function checkPresence(user: string): Promise<void> {
  const client = new MsGraphClient();

  // user can be an email/UPN or a user ID
  const data = (await client.get(`/users/${user}/presence`)) as Presence;

  let line = `${user}: ${data.availability} — ${data.activity}`;
  const statusMsg = data.statusMessage?.message?.content;
  if (statusMsg) {
    line += ` ("${statusMsg}")`;
  }
  process.stdout.write(line + "\n");
}
