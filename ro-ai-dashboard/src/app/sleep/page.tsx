// Asgard Nótt — Sleep Test viewer (ADR-025 S5). Hosts the AirView-style viewer
// (uPlot tracks + minimap + Eir Assistant) which calls /api/sleep/{analyze,chat}
// → mimir-sleep-api. Tenant: asgard_medical (PHI).
export const metadata = { title: "Sleep Test — Asgard Nótt" };

export default function SleepPage() {
  return (
    <div style={{ height: "calc(100vh - 56px)", width: "100%" }}>
      <iframe
        src="/nott/index.html"
        title="Asgard Nótt — Sleep Test"
        style={{ width: "100%", height: "100%", border: "none" }}
      />
    </div>
  );
}
