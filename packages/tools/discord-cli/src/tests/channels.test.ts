import { describe, it, mock } from "node:test";
import assert from "node:assert/strict";

// Inline the pure grouping logic to test without needing a live DiscordClient.
// This mirrors the exact logic in src/commands/channels.ts.

interface DiscordChannel {
  id: string;
  name: string;
  type: number;
  position: number;
  parent_id: string | null;
}

const TEXT_CHANNEL = 0;
const CATEGORY_CHANNEL = 4;

function groupChannels(channels: DiscordChannel[]): Map<string, DiscordChannel[]> {
  const categoryMap = new Map<string, string>();
  for (const ch of channels) {
    if (ch.type === CATEGORY_CHANNEL) {
      categoryMap.set(ch.id, ch.name);
    }
  }

  const textChannels = channels.filter((ch) => ch.type === TEXT_CHANNEL);
  const grouped = new Map<string, DiscordChannel[]>();
  const UNCATEGORIZED = "(uncategorized)";

  for (const ch of textChannels) {
    const categoryName = ch.parent_id
      ? (categoryMap.get(ch.parent_id) ?? UNCATEGORIZED)
      : UNCATEGORIZED;
    if (!grouped.has(categoryName)) {
      grouped.set(categoryName, []);
    }
    grouped.get(categoryName)!.push(ch);
  }

  for (const channelList of grouped.values()) {
    channelList.sort((a, b) => a.position - b.position);
  }

  return grouped;
}

describe("channels grouping", () => {
  const sampleChannels: DiscordChannel[] = [
    { id: "cat1", name: "General", type: 4, position: 0, parent_id: null },
    { id: "cat2", name: "Dev", type: 4, position: 1, parent_id: null },
    { id: "ch1", name: "announcements", type: 0, position: 1, parent_id: "cat1" },
    { id: "ch2", name: "general", type: 0, position: 0, parent_id: "cat1" },
    { id: "ch3", name: "dev-chat", type: 0, position: 0, parent_id: "cat2" },
    { id: "ch4", name: "bot-testing", type: 0, position: 1, parent_id: "cat2" },
    { id: "ch5", name: "random", type: 0, position: 0, parent_id: null },
  ];

  it("groups text channels by their parent category", () => {
    const grouped = groupChannels(sampleChannels);

    assert.ok(grouped.has("General"), "Should have General category");
    assert.ok(grouped.has("Dev"), "Should have Dev category");
    assert.ok(grouped.has("(uncategorized)"), "Should have uncategorized group");
  });

  it("excludes category channels from text channel groups", () => {
    const grouped = groupChannels(sampleChannels);
    const allChannels = [...grouped.values()].flat();

    for (const ch of allChannels) {
      assert.equal(ch.type, TEXT_CHANNEL, "Only text channels should appear in groups");
    }
  });

  it("sorts channels within each category by position ascending", () => {
    const grouped = groupChannels(sampleChannels);

    const generalChannels = grouped.get("General")!;
    assert.equal(generalChannels[0].name, "general", "position 0 should come first");
    assert.equal(generalChannels[1].name, "announcements", "position 1 should come second");

    const devChannels = grouped.get("Dev")!;
    assert.equal(devChannels[0].name, "dev-chat", "position 0 first in Dev");
    assert.equal(devChannels[1].name, "bot-testing", "position 1 second in Dev");
  });

  it("places channels with no parent_id under (uncategorized)", () => {
    const grouped = groupChannels(sampleChannels);
    const uncategorized = grouped.get("(uncategorized)")!;

    assert.equal(uncategorized.length, 1);
    assert.equal(uncategorized[0].name, "random");
  });

  it("returns empty map when no text channels are present", () => {
    const categoryOnly: DiscordChannel[] = [
      { id: "cat1", name: "General", type: 4, position: 0, parent_id: null },
    ];
    const grouped = groupChannels(categoryOnly);
    assert.equal(grouped.size, 0);
  });
});
