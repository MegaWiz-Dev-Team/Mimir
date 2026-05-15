import { useEffect, useState } from "react";
import { getAllTabs, EvalTabId } from "@/components/evaluations/eval-tab-registry";
import { authFetch, API_BASE_URL } from "@/lib/api";

interface TabDataCache {
  [key: string]: any;
}

export function useEvalTabData(activeTab: EvalTabId) {
  const [tabData, setTabData] = useState<TabDataCache>({});
  const [loading, setLoading] = useState<Record<EvalTabId, boolean>>({
    runs: false,
    matrix: false,
    "ai-analysis": false,
    performance: false,
    extraction: false,
    retrieval: false,
    pipeline: false,
    ocr: false,
  });
  const [errors, setErrors] = useState<Record<EvalTabId, string | null>>({
    runs: null,
    matrix: null,
    "ai-analysis": null,
    performance: null,
    extraction: null,
    retrieval: null,
    pipeline: null,
    ocr: null,
  });

  useEffect(() => {
    const tab = getAllTabs().find((t) => t.id === activeTab);
    if (!tab || !tab.endpoint || tabData[activeTab]) {
      return;
    }

    setLoading((prev) => ({ ...prev, [activeTab]: true }));

    authFetch(`${API_BASE_URL}/evaluations/${tab.endpoint}`, { cache: "no-store" })
      .then((response) => {
        if (!response.ok) {
          throw new Error(`HTTP ${response.status}`);
        }
        return response.json();
      })
      .then((data) => {
        setTabData((prev) => ({ ...prev, [activeTab]: data }));
        setErrors((prev) => ({ ...prev, [activeTab]: null }));
      })
      .catch((error) => {
        console.error(`Failed to fetch ${activeTab} data:`, error);
        setErrors((prev) => ({
          ...prev,
          [activeTab]: error instanceof Error ? error.message : "Failed to load data",
        }));
      })
      .finally(() => {
        setLoading((prev) => ({ ...prev, [activeTab]: false }));
      });
  }, [activeTab, tabData]);

  return {
    tabData,
    loading: loading[activeTab] ?? false,
    error: errors[activeTab] ?? null,
  };
}
