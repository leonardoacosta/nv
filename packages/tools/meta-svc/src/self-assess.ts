import { probeFleet, summarizeFleet } from "./health.js";
import type { SelfAssessmentResult } from "./types.js";

const FETCH_TIMEOUT_MS = 3000;
const ASSESSMENT_TIMEOUT_MS = 10000;

interface MemoryTopic {
  topic: string;
  updatedAt?: string;
}

interface RecentMessage {
  channel?: string;
  content?: string;
  createdAt?: string;
}

async function fetchWithTimeout(
  url: string,
  timeoutMs: number,
): Promise<Response> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), timeoutMs);
  try {
    return await fetch(url, { signal: controller.signal });
  } finally {
    clearTimeout(timeout);
  }
}

async function fetchMemoryTopics(): Promise<{
  topics: MemoryTopic[];
  error?: string;
}> {
  try {
    const response = await fetchWithTimeout(
      "http://localhost:4001/api/memory",
      FETCH_TIMEOUT_MS,
    );
    if (!response.ok) {
      return { topics: [], error: `memory-svc returned ${response.status}` };
    }
    const data = (await response.json()) as
      | MemoryTopic[]
      | { result: MemoryTopic[] };
    const topics = Array.isArray(data)
      ? data
      : Array.isArray(data.result)
        ? data.result
        : [];
    return { topics };
  } catch (err) {
    const msg = err instanceof Error ? err.message : "Unknown error";
    return { topics: [], error: `Failed to fetch memory topics: ${msg}` };
  }
}

async function fetchRecentMessages(): Promise<{
  messages: RecentMessage[];
  error?: string;
}> {
  try {
    const response = await fetchWithTimeout(
      "http://localhost:4002/api/messages?per_page=20",
      FETCH_TIMEOUT_MS,
    );
    if (!response.ok) {
      return {
        messages: [],
        error: `messages-svc returned ${response.status}`,
      };
    }
    const data = (await response.json()) as
      | RecentMessage[]
      | { result: RecentMessage[] };
    const messages = Array.isArray(data)
      ? data
      : Array.isArray(data.result)
        ? data.result
        : [];
    return { messages };
  } catch (err) {
    const msg = err instanceof Error ? err.message : "Unknown error";
    return {
      messages: [],
      error: `Failed to fetch recent messages: ${msg}`,
    };
  }
}

function generateObservations(
  topicCount: number,
  messageCount: number,
  fleetHealthy: number,
  fleetTotal: number,
  fleetUnhealthy: number,
  fleetUnreachable: number,
  channelDistribution: Map<string, number>,
  errors: string[],
): string[] {
  const observations: string[] = [];

  observations.push(`${topicCount} memory topic${topicCount !== 1 ? "s" : ""} stored`);
  observations.push(
    `${fleetHealthy}/${fleetTotal} services healthy`,
  );
  observations.push(
    `${messageCount} recent message${messageCount !== 1 ? "s" : ""} retrieved`,
  );

  if (channelDistribution.size > 0) {
    const channelParts: string[] = [];
    for (const [channel, count] of channelDistribution) {
      channelParts.push(`${channel}: ${count}`);
    }
    observations.push(
      `Messages across ${channelDistribution.size} channel${channelDistribution.size !== 1 ? "s" : ""} (${channelParts.join(", ")})`,
    );
  }

  if (fleetUnhealthy > 0) {
    observations.push(`${fleetUnhealthy} service${fleetUnhealthy !== 1 ? "s" : ""} reporting unhealthy`);
  }

  if (fleetUnreachable > 0) {
    observations.push(`${fleetUnreachable} service${fleetUnreachable !== 1 ? "s" : ""} unreachable`);
  }

  for (const error of errors) {
    observations.push(`[partial data] ${error}`);
  }

  return observations;
}

function generateSuggestions(
  topicCount: number,
  messageCount: number,
  fleetHealthy: number,
  fleetTotal: number,
  fleetUnreachable: number,
  errors: string[],
): string[] {
  const suggestions: string[] = [];

  if (fleetUnreachable > 0) {
    suggestions.push(
      `${fleetUnreachable} service${fleetUnreachable !== 1 ? "s" : ""} unreachable -- check systemd status with 'systemctl status nova-tools.target'`,
    );
  }

  if (fleetHealthy < fleetTotal && fleetHealthy > 0) {
    suggestions.push(
      "Some services degraded -- review logs for unhealthy services",
    );
  }

  if (topicCount === 0 && !errors.some((e) => e.includes("memory"))) {
    suggestions.push("No memory topics found -- consider writing initial memory entries");
  }

  if (messageCount === 0 && !errors.some((e) => e.includes("messages"))) {
    suggestions.push("No recent messages -- conversations may not be flowing to messages-svc");
  }

  if (errors.length > 0) {
    suggestions.push(
      "Some data sources were unavailable -- assessment is based on partial data",
    );
  }

  if (suggestions.length === 0) {
    suggestions.push("All systems operational -- no immediate action needed");
  }

  return suggestions;
}

export async function runSelfAssessment(): Promise<SelfAssessmentResult> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), ASSESSMENT_TIMEOUT_MS);

  try {
    const errors: string[] = [];

    const [memoryResult, messagesResult, fleetReports] =
      await Promise.allSettled([
        fetchMemoryTopics(),
        fetchRecentMessages(),
        probeFleet(),
      ]).then((results) =>
        results.map((r) => {
          if (r.status === "fulfilled") return r.value;
          return null;
        }),
      ) as [
        Awaited<ReturnType<typeof fetchMemoryTopics>> | null,
        Awaited<ReturnType<typeof fetchRecentMessages>> | null,
        Awaited<ReturnType<typeof probeFleet>> | null,
      ];

    const topics = memoryResult?.topics ?? [];
    if (memoryResult?.error) errors.push(memoryResult.error);
    if (!memoryResult) errors.push("Failed to gather memory data");

    const messages = messagesResult?.messages ?? [];
    if (messagesResult?.error) errors.push(messagesResult.error);
    if (!messagesResult) errors.push("Failed to gather messages data");

    const fleet = fleetReports ?? [];
    if (!fleetReports) errors.push("Failed to probe fleet health");
    const summary = summarizeFleet(fleet);

    const channelDistribution = new Map<string, number>();
    for (const msg of messages) {
      const channel = msg.channel ?? "unknown";
      channelDistribution.set(
        channel,
        (channelDistribution.get(channel) ?? 0) + 1,
      );
    }

    const observations = generateObservations(
      topics.length,
      messages.length,
      summary.healthy,
      summary.total,
      summary.unhealthy,
      summary.unreachable,
      channelDistribution,
      errors,
    );

    const suggestions = generateSuggestions(
      topics.length,
      messages.length,
      summary.healthy,
      summary.total,
      summary.unreachable,
      errors,
    );

    return {
      generated_at: new Date().toISOString(),
      memory_topic_count: topics.length,
      recent_message_count: messages.length,
      fleet_health: summary,
      observations,
      suggestions,
    };
  } finally {
    clearTimeout(timeout);
  }
}
