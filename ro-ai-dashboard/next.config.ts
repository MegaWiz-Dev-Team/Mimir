import type { NextConfig } from "next";

// Compile-time validation: Prevent Next.js from building static bundles if critical APIs are missing.
if (!process.env.NEXT_PUBLIC_API_URL && process.env.NODE_ENV === 'production') {
  console.warn("⚠️ WARNING: NEXT_PUBLIC_API_URL is not provided during Next.js production build!");
  console.warn("⚠️ The dashboard will fallback to default local ports which WILL result in 502 Bad Gateway.");
  console.warn("⚠️ Please provide this variable inside Docker using: --build-arg NEXT_PUBLIC_API_URL=...");
  // Alternatively, throw an Error here to STRICTLY fail the build and prevent bad deployments:
  // throw new Error("Missing NEXT_PUBLIC_API_URL build argument.");
}

// Baseline CSP — ships in report-only first so we can observe violations before
// enforcing (per Asgard#99 PR-3 plan). 'unsafe-inline'/'unsafe-eval' are required
// by Next.js runtime + Tailwind; tighten when we adopt nonces.
const CSP_REPORT_ONLY = [
  "default-src 'self'",
  "script-src 'self' 'unsafe-inline' 'unsafe-eval'",
  "style-src 'self' 'unsafe-inline'",
  "img-src 'self' data: blob:",
  "font-src 'self' data:",
  "connect-src 'self' https: wss:",
  "frame-ancestors 'none'",
  "base-uri 'self'",
  "form-action 'self'",
].join('; ');

const securityHeaders = [
  // Anti-clickjacking (Odin 2026-06-14 Med finding)
  { key: 'X-Frame-Options', value: 'DENY' },
  // MIME-sniffing protection (Odin Low ×21 hygiene)
  { key: 'X-Content-Type-Options', value: 'nosniff' },
  { key: 'Referrer-Policy', value: 'strict-origin-when-cross-origin' },
  { key: 'Permissions-Policy', value: 'camera=(), microphone=(), geolocation=()' },
  // CSP in report-only — flip to 'Content-Security-Policy' in PR-3 once verified
  { key: 'Content-Security-Policy-Report-Only', value: CSP_REPORT_ONLY },
];

const nextConfig: NextConfig = {
  output: 'standalone',
  async headers() {
    return [{ source: '/:path*', headers: securityHeaders }];
  },
};

export default nextConfig;
