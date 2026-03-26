import type { NextConfig } from "next";

const DAEMON_URL = process.env.DAEMON_URL ?? "http://127.0.0.1:3443";

const nextConfig: NextConfig = {
  output: "standalone",
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
  async rewrites() {
    return [
      {
        source: "/api/:path*",
        destination: `${DAEMON_URL}/api/:path*`,
      },
    ];
  },
};

export default nextConfig;
