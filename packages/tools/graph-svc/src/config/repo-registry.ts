/** Unified project registry — maps ADO and GitHub repos to project codes. */

export interface RepoEntry {
  /** Short project code (e.g., "ws", "oo") */
  code: string;
  /** Human-friendly display name */
  display: string;
  /** Git host provider */
  provider: "ado" | "github";
  /** ADO project name (ADO only — repos live under projects) */
  adoProject?: string;
  /** ADO repo name (may differ from project name) */
  adoRepo?: string;
  /** GitHub org or owner */
  githubOrg?: string;
  /** GitHub repo name */
  githubRepo?: string;
}

export const REPO_REGISTRY: readonly RepoEntry[] = [
  // -- Azure DevOps (brownandbrowninc) --
  { code: "ba", display: "B3", provider: "ado", adoProject: "B3", adoRepo: "B3" },
  { code: "bo", display: "Office Index PIPS", provider: "ado", adoProject: "OfficeIndexToPIPS2.0", adoRepo: "OfficeIndexToPIPS2.0" },
  { code: "dc", display: "Doc Center", provider: "ado", adoProject: "Fireball", adoRepo: "doc" },
  { code: "ew", display: "IaC Hub", provider: "ado", adoProject: "IaC Hub", adoRepo: "IaC-Hub.wiki" },
  { code: "fb", display: "Fireball", provider: "ado", adoProject: "Fireball", adoRepo: "fireball" },
  { code: "ic", display: "Azure Projects", provider: "ado", adoProject: "Azure Projects", adoRepo: "Azure Projects" },
  { code: "lu", display: "Lookups", provider: "ado", adoProject: "Master Data Repository", adoRepo: "lookups" },
  { code: "pp", display: "PIPS", provider: "ado", adoProject: "PIPS", adoRepo: "PIPS" },
  { code: "sc", display: "Sales CRM", provider: "ado", adoProject: "Sales CRM", adoRepo: "Sales CRM" },
  { code: "se", display: "Submission Engine", provider: "ado", adoProject: "Bridge-Summit", adoRepo: "brownandbrown.its.bridge-summit.submission.engine" },
  { code: "tb", display: "The Bridge", provider: "ado", adoProject: "The Bridge", adoRepo: "The Bridge" },
  { code: "ws", display: "Wholesale Architecture", provider: "ado", adoProject: "Wholesale Architecture", adoRepo: "Wholesale Architecture" },

  // -- GitHub (acosta-studio) --
  { code: "cl", display: "Central Leo", provider: "github", githubOrg: "acosta-studio", githubRepo: "central-leo" },
  { code: "co", display: "Claude Orchestrator", provider: "github", githubOrg: "acosta-studio", githubRepo: "claude-orchestrator" },
  { code: "cw", display: "Central Wholesale", provider: "github", githubOrg: "acosta-studio", githubRepo: "central-wholesale" },
  { code: "cx", display: "Cortex", provider: "github", githubOrg: "acosta-studio", githubRepo: "cortex" },
  { code: "hl", display: "Homelab", provider: "github", githubOrg: "acosta-studio", githubRepo: "homelab" },
  { code: "mv", display: "Modern Visa", provider: "github", githubOrg: "acosta-studio", githubRepo: "modern-visa" },
  { code: "oo", display: "Otaku Odyssey", provider: "github", githubOrg: "acosta-studio", githubRepo: "otaku-odyssey" },
  { code: "ss", display: "Styles Silas", provider: "github", githubOrg: "acosta-studio", githubRepo: "styles-silas" },
  { code: "tc", display: "Tribal Cities", provider: "github", githubOrg: "acosta-studio", githubRepo: "tribal-cities" },
  { code: "tl", display: "Tavern Ledger", provider: "github", githubOrg: "acosta-studio", githubRepo: "tavern-ledger" },
  { code: "tm", display: "Terraform Modules", provider: "github", githubOrg: "acosta-studio", githubRepo: "terraform-modules" },

  // -- GitHub (leonardoacosta) --
  { code: "cc", display: "Central Claude", provider: "github", githubOrg: "leonardoacosta", githubRepo: "central-claude" },
  { code: "if", display: "Installfest", provider: "github", githubOrg: "leonardoacosta", githubRepo: "Installfest" },
  { code: "la", display: "Leonardo Acosta", provider: "github", githubOrg: "leonardoacosta", githubRepo: "leonardoacostaNextJs" },
  { code: "nv", display: "Nova", provider: "github", githubOrg: "leonardoacosta", githubRepo: "nv" },
  { code: "nx", display: "Nexus", provider: "github", githubOrg: "leonardoacosta", githubRepo: "nexus" },
  { code: "sj", display: "Seth Jones", provider: "github", githubOrg: "leonardoacosta", githubRepo: "SethAJones" },

  // -- GitHub (Priceless-Development) --
  { code: "ct", display: "Civalent", provider: "github", githubOrg: "Priceless-Development", githubRepo: "civalent" },
  { code: "lv", display: "Las Vegas Promotions", provider: "github", githubOrg: "Priceless-Development", githubRepo: "LasVegasClubPromotions" },

  // -- GitHub (leonardoacosta fork) --
  { code: "fp", display: "Civalent Fork", provider: "github", githubOrg: "leonardoacosta", githubRepo: "civalent" },
] as const;

// -- Lookup helpers --

/** Build lookup indices for O(1) access. */
const adoRepoIndex = new Map<string, RepoEntry>();
const adoProjectIndex = new Map<string, RepoEntry>();

for (const entry of REPO_REGISTRY) {
  if (entry.provider === "ado") {
    if (entry.adoRepo) adoRepoIndex.set(entry.adoRepo.toLowerCase(), entry);
    if (entry.adoProject) adoProjectIndex.set(entry.adoProject.toLowerCase(), entry);
  }
}

/**
 * Look up a repo entry by ADO repository name and optional project name.
 * Matches on repo name first (exact), falls back to project name.
 */
export function lookupAdoRepo(repoName?: string, adoProject?: string): RepoEntry | undefined {
  if (repoName) {
    const byRepo = adoRepoIndex.get(repoName.toLowerCase());
    if (byRepo) return byRepo;
  }
  if (adoProject) {
    const byProject = adoProjectIndex.get(adoProject.toLowerCase());
    if (byProject) return byProject;
  }
  return undefined;
}
