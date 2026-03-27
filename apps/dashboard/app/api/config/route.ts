import { NextResponse } from "next/server";
import type { ConfigGetResponse } from "@/types/api";

export async function GET() {
  const config: ConfigGetResponse = {
    tool_router_url: process.env.TOOL_ROUTER_URL ?? "http://host.docker.internal:4100",
    memory_svc_url: process.env.MEMORY_SVC_URL ?? "http://host.docker.internal:4101",
    messages_svc_url: process.env.MESSAGES_SVC_URL ?? "http://host.docker.internal:4102",
    meta_svc_url: process.env.META_SVC_URL ?? "http://host.docker.internal:4108",
    nv_projects: process.env.NV_PROJECTS ?? "[]",
  };

  return NextResponse.json(config);
}

export async function PUT() {
  return NextResponse.json(
    {
      error: "Configuration changes should be made via environment variables",
      note: "Update docker-compose.yml environment block and redeploy",
    },
    { status: 501 },
  );
}
