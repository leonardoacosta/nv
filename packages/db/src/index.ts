export { db } from "./client.js";

export { messages } from "./schema/messages.js";
export type { Message, NewMessage } from "./schema/messages.js";

export { obligations } from "./schema/obligations.js";
export type { Obligation, NewObligation } from "./schema/obligations.js";

export { contacts } from "./schema/contacts.js";
export type { Contact, NewContact } from "./schema/contacts.js";

export { diary } from "./schema/diary.js";
export type { DiaryEntry, NewDiaryEntry } from "./schema/diary.js";

export { memory } from "./schema/memory.js";
export type { Memory, NewMemory } from "./schema/memory.js";

export { briefings } from "./schema/briefings.js";
export type { Briefing, NewBriefing } from "./schema/briefings.js";

export { reminders } from "./schema/reminders.js";
export type { Reminder, NewReminder } from "./schema/reminders.js";

export { schedules } from "./schema/schedules.js";
export type { Schedule, NewSchedule } from "./schema/schedules.js";

export { sessions } from "./schema/sessions.js";
export type { Session, NewSession } from "./schema/sessions.js";

export { sessionEvents } from "./schema/session-events.js";
export type { SessionEvent, NewSessionEvent } from "./schema/session-events.js";

export { projects } from "./schema/projects.js";
export type { Project, NewProject } from "./schema/projects.js";
export {
  projectCategoryEnum,
  projectStatusEnum,
  createProjectSchema,
  updateProjectSchema,
} from "./schema/projects.js";
export type {
  ProjectCategory,
  ProjectStatus,
  CreateProjectInput,
  UpdateProjectInput,
} from "./schema/projects.js";
