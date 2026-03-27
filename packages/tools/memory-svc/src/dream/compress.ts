/**
 * LLM compression for oversize memory topics.
 *
 * memory-svc does NOT depend on the Agent SDK. Instead, the caller (daemon
 * orchestrator) injects a `compressor` callback that wraps Agent SDK query().
 */

/**
 * Compress a single topic using an LLM via the injected compressor callback.
 *
 * @param topic   - The topic name (for prompt context).
 * @param content - The raw topic content to compress.
 * @param targetKb - Target size ceiling in KB.
 * @param compressor - Callback that sends a prompt string to an LLM and
 *                     returns the response text. The daemon injects its
 *                     Agent SDK here.
 * @returns Compressed content string, or null if compression failed/unavailable.
 */
export async function compressTopic(
  topic: string,
  content: string,
  targetKb: number,
  compressor: (prompt: string) => Promise<string>,
): Promise<string | null> {
  const systemPrompt = [
    "You are a memory compressor.",
    `Compress the following memory topic "${topic}" to under ${targetKb}KB.`,
    "Preserve: recent decisions, active projects, key relationships, dates, names, technical details.",
    "Remove: stale context, resolved issues, outdated patterns, redundant information.",
    "Output only the compressed content -- no preamble, no explanation.",
  ].join(" ");

  const prompt = `${systemPrompt}\n\n---\n\n${content}`;

  try {
    const result = await Promise.race([
      compressor(prompt),
      new Promise<never>((_, reject) =>
        setTimeout(() => reject(new Error("LLM compression timed out after 60s")), 60_000),
      ),
    ]);
    return result;
  } catch {
    // LLM unavailable or timed out -- caller keeps rules-phase result
    return null;
  }
}
