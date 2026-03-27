import { redirect } from "next/navigation";

/**
 * /approvals now redirects to /obligations?tab=approvals
 * for backward compatibility after the merge.
 */
export default function ApprovalsPage() {
  redirect("/obligations?tab=approvals");
}
