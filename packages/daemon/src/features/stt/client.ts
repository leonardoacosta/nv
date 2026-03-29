/**
 * ElevenLabs Speech-to-Text client.
 * Provides transcribe(fileUrl) for converting Telegram voice messages to text.
 */

const ELEVENLABS_STT_URL = "https://api.elevenlabs.io/v1/speech-to-text/convert";
const DOWNLOAD_TIMEOUT_MS = 30_000;
const TRANSCRIBE_TIMEOUT_MS = 30_000;

// ─── SttError ─────────────────────────────────────────────────────────────────

export type SttErrorCode =
  | "missing_key"
  | "download_failed"
  | "api_error"
  | "empty_transcript";

export class SttError extends Error {
  constructor(
    public readonly code: SttErrorCode,
    message: string,
  ) {
    super(message);
    this.name = "SttError";
  }
}

// ─── transcribe ───────────────────────────────────────────────────────────────

/**
 * Transcribe a voice file from a Telegram CDN URL using ElevenLabs STT.
 *
 * @param fileUrl - Telegram CDN URL for the voice message (OGG/Opus format).
 * @returns The transcribed text string.
 * @throws SttError with code "missing_key" if ELEVENLABS_API_KEY is not set.
 * @throws SttError with code "download_failed" if the audio download fails.
 * @throws SttError with code "api_error" if the ElevenLabs API returns non-2xx.
 * @throws SttError with code "empty_transcript" if the transcript is blank.
 */
export async function transcribe(fileUrl: string): Promise<string> {
  // Read API key at call time (not module load) so tests can override env
  const apiKey = process.env["ELEVENLABS_API_KEY"];
  if (!apiKey) {
    throw new SttError("missing_key", "ELEVENLABS_API_KEY not set");
  }

  // Download audio bytes from Telegram CDN
  let audioBytes: ArrayBuffer;
  try {
    const downloadController = new AbortController();
    const downloadTimeout = setTimeout(
      () => downloadController.abort(),
      DOWNLOAD_TIMEOUT_MS,
    );

    let downloadResponse: Response;
    try {
      downloadResponse = await fetch(fileUrl, {
        signal: downloadController.signal,
      });
    } finally {
      clearTimeout(downloadTimeout);
    }

    if (!downloadResponse.ok) {
      throw new SttError(
        "download_failed",
        `Audio download failed: HTTP ${downloadResponse.status} ${downloadResponse.statusText}`,
      );
    }

    audioBytes = await downloadResponse.arrayBuffer();
  } catch (err: unknown) {
    if (err instanceof SttError) {
      throw err;
    }
    const message =
      err instanceof Error ? err.message : String(err);
    throw new SttError("download_failed", `Audio download error: ${message}`);
  }

  // POST raw bytes to ElevenLabs STT
  let transcript: string;
  try {
    const transcribeController = new AbortController();
    const transcribeTimeout = setTimeout(
      () => transcribeController.abort(),
      TRANSCRIBE_TIMEOUT_MS,
    );

    let sttResponse: Response;
    try {
      sttResponse = await fetch(ELEVENLABS_STT_URL, {
        method: "POST",
        headers: {
          "xi-api-key": apiKey,
          "Content-Type": "audio/ogg",
        },
        body: audioBytes,
        signal: transcribeController.signal,
      });
    } finally {
      clearTimeout(transcribeTimeout);
    }

    if (!sttResponse.ok) {
      throw new SttError(
        "api_error",
        `ElevenLabs STT API error: HTTP ${sttResponse.status} ${sttResponse.statusText}`,
      );
    }

    const json = (await sttResponse.json()) as { text?: string };
    transcript = json.text ?? "";
  } catch (err: unknown) {
    if (err instanceof SttError) {
      throw err;
    }
    const message =
      err instanceof Error ? err.message : String(err);
    throw new SttError("api_error", `ElevenLabs STT request error: ${message}`);
  }

  if (!transcript.trim()) {
    throw new SttError("empty_transcript", "ElevenLabs STT returned an empty transcript");
  }

  return transcript;
}
