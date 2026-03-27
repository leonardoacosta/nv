import type { NextConfig } from "next";

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
};

export default nextConfig;
