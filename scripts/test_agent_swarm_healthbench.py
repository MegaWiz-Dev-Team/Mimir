#!/usr/bin/env python3
"""Offline checks for agent_swarm_healthbench.py.

  python3 -m unittest scripts/test_agent_swarm_healthbench.py

The replay test feeds the 2026-05-30 KB A/B rows (fixtures/) through
summarize(). The pre-fix harness scored 22 HTTP 500 replies as answers and
reported the all-3-KB arm 16pp below RAG-only; counted as errors, the paired
difference on answered items is +1.1pp.
"""
import json, os, statistics, sys, unittest

sys.path.insert(0, os.path.dirname(__file__))
from agent_swarm_healthbench import classify, judge_averaged, no_thinking, summarize  # noqa: E402

FIXTURE = os.path.join(os.path.dirname(__file__), "fixtures", "kb_ab_replay_2026-05-30.json")


class Classify(unittest.TestCase):
    def test_bifrost_error_is_not_an_answer(self):
        self.assertEqual(classify("(error: HTTP Error 500: Internal Server Error)")[0], "error")

    def test_tool_request_stub_is_not_an_answer(self):
        stub = '{\n  "action_required": {\n    "action_type": "graph_search"\n  }\n}'
        self.assertEqual(classify(stub)[0], "stub")

    def test_unparseable_stub_is_still_a_stub(self):
        self.assertEqual(classify('{"action_required": {"action_type": "x"} trailing')[0], "stub")

    def test_envelope_with_answer_is_judged_on_the_answer(self):
        outcome, text = classify('{"action_required": null, "final_answer": "Rest and ice."}')
        self.assertEqual((outcome, text), ("ok", "Rest and ice."))

    def test_envelope_without_answer_is_empty(self):
        self.assertEqual(classify('{"action_required": null, "final_answer": ""}')[0], "empty")

    def test_blank_reply_is_empty(self):
        self.assertEqual(classify("  \n")[0], "empty")

    def test_plain_text_is_an_answer(self):
        self.assertEqual(classify("Take the medication with food."), ("ok", "Take the medication with food."))


class JudgeConfig(unittest.TestCase):
    def test_gemini_3_turns_thinking_off_by_level(self):
        self.assertEqual(no_thinking("gemini-3.5-flash-lite"), {"thinkingLevel": "minimal"})

    def test_gemini_2_turns_thinking_off_by_budget(self):
        self.assertEqual(no_thinking("gemini-2.5-flash"), {"thinkingBudget": 0})


class JudgeAveraged(unittest.TestCase):
    def fake_judge(self, scores):
        calls = []
        def judge(prompt, text, rubrics):
            calls.append(text)
            pct, harmful = scores[len(calls) - 1]
            return pct, pct * 10, harmful
        return judge, calls

    def test_mean_of_reps_and_majority_harmful(self):
        judge, calls = self.fake_judge([(0.2, True), (0.5, False), (0.8, True)])
        pct, got, harmful, reps = judge_averaged("p", "answer", [], 3, {}, "item", judge)
        self.assertAlmostEqual(pct, 0.5)
        self.assertAlmostEqual(got, 5.0)
        self.assertTrue(harmful)
        self.assertEqual(len(calls), 3)

    def test_identical_answer_to_the_same_item_is_judged_once(self):
        judge, calls = self.fake_judge([(0.4, False), (0.9, False)])
        cache = {}
        first = judge_averaged("p", "same text", [], 1, cache, "item", judge)
        second = judge_averaged("p", "same text", [], 1, cache, "item", judge)
        self.assertEqual(first, second)
        self.assertEqual(len(calls), 1)

    def test_same_text_for_another_item_is_judged_again(self):
        judge, calls = self.fake_judge([(0.4, False), (0.9, False)])
        cache = {}
        judge_averaged("p", "same text", [], 1, cache, "item-1", judge)
        judge_averaged("p", "same text", [], 1, cache, "item-2", judge)
        self.assertEqual(len(calls), 2)


class ReplayKbAb(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        with open(FIXTURE) as f:
            cls.records = json.load(f)["records"]
        cls.summary = summarize(cls.records, ["rag-only", "all3-kb"])

    def test_errors_are_counted_not_scored(self):
        all3 = self.summary["arms"]["all3-kb"]
        self.assertEqual(all3["outcomes"]["error"], 22)
        self.assertEqual(all3["outcomes"]["ok"], 58)
        self.assertAlmostEqual(all3["answer_rate"], 58 / 80)

    def test_old_method_reproduces_the_reported_regression(self):
        def naive(arm, slot):
            return statistics.fmean(r["old_pct"] for r in self.records if r["arm"] == arm and r["slot"] == slot)
        im = "eir-internal-medicine"
        self.assertAlmostEqual(naive("all3-kb", im), 0.414, places=3)
        self.assertAlmostEqual(naive("rag-only", im), 0.574, places=3)

    def test_paired_on_answered_items_shows_no_difference(self):
        p = self.summary["paired"]["all3-kb - rag-only"]
        self.assertEqual(p["n"], 58)
        self.assertAlmostEqual(p["mean_delta"], 0.011, delta=0.001)
        self.assertEqual((p["wins"], p["losses"]), (16, 14))
        low, high = p["ci95"]
        self.assertLess(low, 0)
        self.assertGreater(high, 0)


if __name__ == "__main__":
    unittest.main()
