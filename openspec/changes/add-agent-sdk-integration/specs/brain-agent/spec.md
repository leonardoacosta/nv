# Spec: NovaAgent — Agent SDK Brain

## ADDED Requirements

### Requirement: NovaAgent processMessage

`NovaAgent` MUST expose `processMessage(message: Message, history: Message[]): Promise<AgentResponse>` which invokes `query()` from `@anthropic-ai/claude-agent-sdk` and returns a structured `AgentResponse`.

#### Scenario: Successful round-trip

Given `NovaAgent` is constructed with a valid config and an existing `config/system-prompt.md`,
and `processMessage` is called with a `Message` containing `content: "ping"` and an empty history array,
when the SDK `query()` resolves with a `ResultMessage`,
then `AgentResponse.text` is a non-empty string, `AgentResponse.stopReason` is `"end_turn"`, and `AgentResponse.toolCalls` is an array (possibly empty).

#### Scenario: Tool calls are collected

Given a `query()` run that exercises one tool call before `end_turn`,
when `processMessage` completes,
then `AgentResponse.toolCalls` contains one entry with `name`, `input`, and `result` fields populated from the SDK event stream.

#### Scenario: Gateway key missing

Given `VERCEL_GATEWAY_KEY` is unset and `config.vercelGatewayKey` is undefined,
when `processMessage` is called,
then it throws `Error: "VERCEL_GATEWAY_KEY is required but not set"` before calling `query()`.

### Requirement: System prompt loading

`NovaAgent` SHALL load `config/system-prompt.md` at construction time and MUST fall back to an empty string with a warning log if the file is absent.

#### Scenario: File present

Given `config/system-prompt.md` exists and contains text `"You are Nova."`,
when `NovaAgent` is constructed,
then `agent.systemPrompt` equals `"You are Nova."`.

#### Scenario: File missing

Given `config/system-prompt.md` does not exist,
when `NovaAgent` is constructed,
then a warning is logged and `agent.systemPrompt` is `""` (no throw).
