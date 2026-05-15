export const VERSION = {
  version: "2026-05-16-v2.3.10",
  buildDate: new Date().toISOString(),
  features: ["OCR-Benchmark", "Confidence-Display", "Menu-Reorganized", "Tenant-Config", "Tenant-Auth", "Eval-All-Types"],
  components: {
    mimir: "2.3.0",
    dashboard: "2.3.0",
    api: "0.3.0"
  }
};

export function logVersion() {
  const msg = `
╔════════════════════════════════════════════════════════╗
║        🚀 MIMIR Dashboard v${VERSION.version}        ║
║════════════════════════════════════════════════════════║
║  Build: ${VERSION.buildDate}
║  Features: ${VERSION.features.join(", ")}
║  Components:
║    • Mimir API: ${VERSION.components.mimir}
║    • Dashboard: ${VERSION.components.dashboard}
║    • Syn API: ${VERSION.components.api}
╚════════════════════════════════════════════════════════╝
  `;
  console.log(msg);
  console.log("%cMimir Dashboard Ready", "color: #00ff00; font-weight: bold; font-size: 14px;");
}
