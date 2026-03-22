# Spec: Config Population

## MODIFIED Requirements

### Requirement: Populate user.md

The system MUST replace all "(discovered during bootstrap)" placeholders in `config/user.md`
with actual operator details: name, timezone, notification level, work context, communication
preferences, and decision patterns.

#### Scenario: System prompt includes real user context

**Given** the daemon loads `~/.nv/user.md` into the system prompt
**When** Claude reads the user context section
**Then** it sees concrete details (name: Leo, timezone: America/Chicago, notification level:
everything noteworthy) instead of placeholder stubs
**And** Claude does NOT trigger first-session/onboarding behavior

### Requirement: Populate identity.md

The system MUST replace placeholder emoji and avatar fields in `config/identity.md` with actual values.

#### Scenario: Nova identity is complete

**Given** the daemon loads `~/.nv/identity.md` into the system prompt
**When** Claude reads the identity section
**Then** emoji is "✨" and avatar description is populated
**And** no "(discovered during bootstrap)" text remains
