# Proposal: Fix Telegram Null Callback

## Change ID
`fix-telegram-null-callback`

## Summary

When Leo taps Approve / Edit / Cancel inline keyboard buttons, Telegram shows a
"null" bubble. The `answerCallbackQuery` call in `poll_messages()` passes `None`
as the notification text; Telegram renders that as the literal string "null".
Fix by passing a short, action-specific string derived from the callback data prefix.

## Context
- File: `crates/nv-daemon/src/channels/telegram/mod.rs`
- Line ~90: `self.client.answer_callback_query(&cb.id, None).await`

## Motivation

Every button tap produces a jarring "null" toast. It is the first thing Leo sees
after confirming an action. One-line fix, zero architectural change.

## Requirements

### Req-1: Map callback data prefix to response text

In `poll_messages()`, replace `None` with `Some(text)` where `text` is derived
from the callback data:

| Prefix | Text |
|--------|------|
| `approve:` | `"Working on it..."` |
| `edit:` | `"Editing..."` |
| `cancel:` | `"Cancelled."` |
| `action:` | `"Got it."` |
| _(anything else)_ | `"Got it."` |

## Scope
- **IN**: `answer_callback_query` call site in `poll_messages()`
- **OUT**: callback routing logic, action execution, any other file

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/channels/telegram/mod.rs` | Replace `None` with `Some(label)` in `answer_callback_query` call |

## Risks
None — `answerCallbackQuery` text is display-only and does not affect routing.
