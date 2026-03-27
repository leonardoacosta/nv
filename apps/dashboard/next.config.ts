import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  output: "standalone",
  // Trace workspace dependencies so they're included in the standalone bundle.
  // Without this, @nova/db (and its deps like drizzle-orm, postgres) are missing
  // at runtime in the Docker container.
  outputFileTracingRoot: require("path").join(__dirname, "../../"),
  serverExternalPackages: ["postgres"],
  async redirects() {
    return [
      {
        source: "/nexus",
        destination: "/sessions",
        permanent: true,
      },
      {
        source: "/cold-starts",
        destination: "/usage?tab=performance",
        permanent: true,
      },
      {
        source: "/session",
        destination: "/sessions?panel=cc",
        permanent: true,
      },
    ];
  },
};

export default nextConfig;
