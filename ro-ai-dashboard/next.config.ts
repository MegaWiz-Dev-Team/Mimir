import type { NextConfig } from "next";

// Compile-time validation: Prevent Next.js from building static bundles if critical APIs are missing.
if (!process.env.NEXT_PUBLIC_API_URL && process.env.NODE_ENV === 'production') {
  console.warn("⚠️ WARNING: NEXT_PUBLIC_API_URL is not provided during Next.js production build!");
  console.warn("⚠️ The dashboard will fallback to default local ports which WILL result in 502 Bad Gateway.");
  console.warn("⚠️ Please provide this variable inside Docker using: --build-arg NEXT_PUBLIC_API_URL=...");
  // Alternatively, throw an Error here to STRICTLY fail the build and prevent bad deployments:
  // throw new Error("Missing NEXT_PUBLIC_API_URL build argument.");
}

// Enforcing CSP (Asgard#99 PR-3 — flipped from report-only to enforcing).
//
// script-src keeps 'unsafe-inline' but DROPS 'unsafe-eval': this app is statically
// pre-rendered (Next.js bakes inline RSC-payload <script> tags at build with no
// per-request nonce), so a nonce/strict-dynamic policy would block its own bootstrap
// scripts. Removing 'unsafe-eval' still blocks eval()/Function() XSS — production
// Next.js does not need eval. style-src keeps 'unsafe-inline' (Tailwind/Next inject
// inline <style>; not a script-execution vector). The remaining directives
// (object-src/base-uri/form-action 'self', frame-ancestors 'none') are real hardening.
// Fully removing script 'unsafe-inline' requires app-wide dynamic rendering — deferred.
const CSP = [
  "default-src 'self'",
  "script-src 'self' 'unsafe-inline'",
  "style-src 'self' 'unsafe-inline'",
  "img-src 'self' data: blob:",
  "font-src 'self' data:",
  "connect-src 'self' https: wss:",
  "frame-ancestors 'none'",
  "base-uri 'self'",
  "form-action 'self'",
  "object-src 'none'",
].join('; ');

const securityHeaders = [
  // Anti-clickjacking (Odin 2026-06-14 Med finding) — CSP frame-ancestors also covers this.
  { key: 'X-Frame-Options', value: 'DENY' },
  // MIME-sniffing protection (Odin Low ×21 hygiene)
  { key: 'X-Content-Type-Options', value: 'nosniff' },
  { key: 'Referrer-Policy', value: 'strict-origin-when-cross-origin' },
  { key: 'Permissions-Policy', value: 'camera=(), microphone=(), geolocation=()' },
  { key: 'Content-Security-Policy', value: CSP },
];

const nextConfig: NextConfig = {
  output: 'standalone',
  async headers() {
    return [{ source: '/:path*', headers: securityHeaders }];
  },
};

export default nextConfig;
