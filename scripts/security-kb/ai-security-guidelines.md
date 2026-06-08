# Asgard AI & Cyber Security Guidelines / แนวปฏิบัติความมั่นคงปลอดภัย AI

Reference knowledge base for the Asgard SOC agents (Odin / Huginn / Muninn / Loki / Týr).
Sources: OWASP Top 10 for LLM Applications (2025), OWASP Web Top 10 (2021), MITRE ATT&CK, CWE,
NIST SP 800-53 / AI RMF, secure-AI-lifecycle practice. Each section is self-contained for RAG retrieval.

---

## LLM01 — Prompt Injection / การโจมตีด้วยการแทรกคำสั่ง
Prompt injection occurs when attacker-controlled text overrides the model's intended instructions —
either directly (user input) or indirectly (poisoned documents, web pages, tool output the model reads).
Impact: data exfiltration, unauthorized tool calls, policy bypass, social-engineering of downstream systems.
Controls: treat all model input as untrusted; enforce least-privilege on tools the model can call; keep a
non-bypassable system policy outside the prompt; sandbox/allowlist tool actions; require human approval for
high-impact actions; detect injection with a guardrail layer (e.g., Skuggi/PII gate) and log to the SIEM
(Týr). Never concatenate untrusted content directly into a system prompt.

## LLM02 — Sensitive Information Disclosure / การรั่วไหลของข้อมูลอ่อนไหว
The model reveals PII, secrets, credentials, proprietary data, or other tenants' data in its output.
Causes: over-broad RAG retrieval, secrets in training/fine-tune data, cross-tenant leakage, verbose errors.
Controls: PII detection + masking before send (mask-and-send / encrypt modes); strict tenant isolation on
retrieval (filter by tenant_id); scrub secrets from logs and errors; output filtering; data minimization;
never log raw tokens or credentials.

## LLM03 — Supply Chain / ห่วงโซ่อุปทาน
Compromised models, datasets, plugins, or dependencies. Risks: backdoored model weights, poisoned public
datasets, malicious packages, vulnerable libraries (SCA). Controls: pin and verify model + dataset
provenance and hashes; scan dependencies (cargo audit, trivy, semgrep); vendor allowlists; SBOM; verify
signatures; isolate untrusted plugins.

## LLM04 — Data & Model Poisoning / การวางยาข้อมูลและโมเดล
Manipulating training/fine-tune/RAG data to implant backdoors or bias. Controls: curate and sign training
data; clean-room provenance; anomaly detection on datasets; restrict who can write to RAG sources; review
ingested documents; monitor for sudden behavior shifts.

## LLM05 — Improper Output Handling / การจัดการเอาต์พุตไม่ปลอดภัย
Downstream systems trust LLM output blindly → XSS, SQLi, SSRF, command injection, path traversal. Controls:
treat model output as untrusted; context-aware encoding; parameterized queries; validate/allowlist before
executing any model-suggested action, URL, or code.

## LLM06 — Excessive Agency / สิทธิ์เกินจำเป็นของ Agent
Agents granted too much permission, autonomy, or tool access. Controls: least-privilege tools; scoped,
revocable credentials; human-in-the-loop gates for writes/enforcement (e.g., Thor policy gate before merge /
active-response); rate limits; full audit trail of every tool call.

## LLM07 — System Prompt Leakage / การรั่วไหลของ System Prompt
Attackers extract the system prompt, revealing logic, guardrails, or embedded secrets. Controls: never put
secrets/keys in prompts; assume the prompt is discoverable; enforce authorization in code, not in the prompt.

## LLM08 — Vector & Embedding Weaknesses / จุดอ่อนของ Vector/Embedding
RAG-specific: embedding inversion (reconstruct source text), cross-tenant retrieval, poisoned vectors.
Controls: tenant_id filtering on every vector query; access control on collections; validate ingested
content; monitor retrieval for anomalies.

## LLM09 — Misinformation / ข้อมูลผิดพลาด (Hallucination)
Confident but wrong output causing harmful decisions. Controls: ground answers in retrieved, cited sources
(RAG) and say "unknown" when unsupported; human review for high-stakes domains (clinical, financial);
provenance + confidence on outputs.

## LLM10 — Unbounded Consumption / การใช้ทรัพยากรไม่จำกัด
Resource-exhaustion / cost / denial-of-wallet via expensive queries or loops. Controls: per-tenant token and
rate quotas (max_daily_tokens); timeouts and wall-clock bounds on tool runs; circuit breakers; monitor spend.

---

## WEB — Common application findings (DAST/ZAP, what they mean)
- **Cross-Domain Misconfiguration (CORS `*`)**: `Access-Control-Allow-Origin: *` lets any origin read
  responses. Fix: restrict to an explicit allowlist; never combine `*` with credentials.
- **Content Security Policy (CSP) missing/weak**: absent CSP, wildcard sources, or `unsafe-inline` enable
  XSS. Fix: a strict CSP with nonces/hashes, no `unsafe-inline`, explicit `default-src`.
- **Missing Anti-clickjacking header**: no `X-Frame-Options`/`frame-ancestors` → clickjacking. Fix:
  `X-Frame-Options: DENY` or CSP `frame-ancestors 'none'`.
- **X-Content-Type-Options missing**: MIME sniffing. Fix: `X-Content-Type-Options: nosniff`.
- **Cookie without HttpOnly / Secure**: theft via XSS / over plaintext. Fix: set `HttpOnly`, `Secure`,
  `SameSite`.
- **Server version disclosure**: `Server`/`X-Powered-By` aids fingerprinting. Fix: suppress version banners.
- **Application Error Disclosure**: stack traces/messages leak internals. Fix: generic errors to users, full
  detail only in server logs.

## INJECTION — SQLi / Command / Path traversal
Untrusted input reaches an interpreter. Fix: parameterized queries / prepared statements; never build SQL or
shell from user input; allowlist + canonicalize file paths; run with least privilege.

---

## Secure-AI lifecycle controls / วงจรพัฒนา AI ปลอดภัย
1. **Design**: threat-model the AI feature; classify data; define trust boundaries; least privilege by default.
2. **Data**: provenance + signing; PII handling and minimization; clean-room for licensed/research data.
3. **Build/Train**: pin model + dependency versions; scan (SAST/SCA); verify weights and dataset hashes.
4. **Deploy**: on-prem / tenant isolation; secrets in a vault, never in prompts or images; read-only rootfs.
5. **Operate**: continuous VA scanning (Huginn), red-team simulation (Loki), SIEM detection (Týr), automated
   remediation (Muninn), policy enforcement (Thor). Purple-team loop: Loki attacks → Týr detects.
6. **Govern**: audit every privileged/agent action; human-in-the-loop for high-impact changes; incident
   response runbook; periodic review.

## Asgard red/blue mapping
- **Offensive (red side)**: Huginn = VA assessment (breadth, non-exploitative); Loki = pen-test/red-team
  (exploitation, evasion).
- **Defensive (blue side)**: Týr = SIEM detection; Muninn = auto-remediation; Várðr = monitoring.
- **Purple**: Loki → Týr (attack → detect); Huginn finding → Muninn (find → fix).
- **Enforcement**: Thor = policy-as-code gate (fail-closed) in front of merges / active-response.
