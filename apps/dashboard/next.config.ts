import type { NextConfig } from "next";

const DAEMON_URL = process.env.DAEMON_URL ?? "http://127.0.0.1:3443";

const nextConfig: NextConfig = {
  output: "standalone",
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
