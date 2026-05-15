import type { NextConfig } from "next";

// Compile-time validation: Prevent Next.js from building static bundles if critical APIs are missing.
if (!process.env.NEXT_PUBLIC_API_URL && process.env.NODE_ENV === 'production') {
  console.warn("⚠️ WARNING: NEXT_PUBLIC_API_URL is not provided during Next.js production build!");
  console.warn("⚠️ The dashboard will fallback to default local ports which WILL result in 502 Bad Gateway.");
  console.warn("⚠️ Please provide this variable inside Docker using: --build-arg NEXT_PUBLIC_API_URL=...");
  // Alternatively, throw an Error here to STRICTLY fail the build and prevent bad deployments:
  // throw new Error("Missing NEXT_PUBLIC_API_URL build argument.");
}

const nextConfig: NextConfig = {
  output: 'standalone',
  async rewrites() {
    return {
      beforeFiles: [
        {
          source: '/api/v1/:path*',
          destination: `${process.env.NEXT_PUBLIC_API_URL}/api/v1/:path*`,
        },
      ],
    };
  },
};

export default nextConfig;
