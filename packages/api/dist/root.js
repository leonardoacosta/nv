import { createTRPCRouter, createCallerFactory } from "./trpc.js";
import { obligationRouter } from "./routers/obligation.js";
import { contactRouter } from "./routers/contact.js";
import { diaryRouter } from "./routers/diary.js";
import { briefingRouter } from "./routers/briefing.js";
import { messageRouter } from "./routers/message.js";
import { sessionRouter } from "./routers/session.js";
import { automationRouter } from "./routers/automation.js";
import { systemRouter } from "./routers/system.js";
import { authRouter } from "./routers/auth.js";
import { projectRouter } from "./routers/project.js";
/**
 * Root tRPC router merging all 10 domain routers.
 *
 * Dashboard-local routers (cc-session, resolve) are merged at the
 * catch-all handler in apps/dashboard, not here.
 */
export const appRouter = createTRPCRouter({
    obligation: obligationRouter,
    contact: contactRouter,
    diary: diaryRouter,
    briefing: briefingRouter,
    message: messageRouter,
    session: sessionRouter,
    automation: automationRouter,
    system: systemRouter,
    auth: authRouter,
    project: projectRouter,
});
/** Server-side caller factory. */
export const createCaller = createCallerFactory(appRouter);
//# sourceMappingURL=root.js.map