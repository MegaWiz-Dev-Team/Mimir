# ESI Triage Algorithm (Emergency Severity Index)

## Overview

The **Emergency Severity Index (ESI)** is a five-level emergency department (ED) triage algorithm designed to rapidly identify patients who require immediate emergency interventions or close monitoring.

**Version:** ESI-4 (2012)  
**Category:** Emergency Medicine  
**Specialty:** Emergency Department Triage  
**Updated:** 2026-05-15

---

## Quick Reference

| Level | Name | Description | Response Time |
|-------|------|-------------|----------------|
| **ESI-1** | Resuscitation | Requires immediate life-saving intervention | Immediate |
| **ESI-2** | Emergency | High-risk situation, requires immediate evaluation | <10 minutes |
| **ESI-3** | Urgent | Stable, but requires urgent evaluation | <30 minutes |
| **ESI-4** | Semi-Urgent | Stable, may be discharged to waiting room | 1-2 hours |
| **ESI-5** | Non-Urgent | Minor problem, minimal risk | 2-4 hours |

---

## ESI Algorithm Flow

```
↓ Is patient in DANGER?
├─ YES → ESI-1 (Resuscitation)
└─ NO
   ↓ Requires HIGH-RISK situation?
   ├─ YES → ESI-2 (Emergency)
   └─ NO
      ↓ Requires immediate assessment?
      ├─ YES → ESI-3 (Urgent)
      └─ NO
         ↓ Multiple resources needed?
         ├─ YES → ESI-4 (Semi-Urgent)
         └─ NO → ESI-5 (Non-Urgent)
```

---

## Decision Rules

### Rule 1: Is the patient in immediate danger?

**ESI-1 Criteria:**
- Requires immediate life-saving intervention
- Examples:
  - Severe sepsis with altered mental status
  - Acute stroke (potential thrombolytic candidate)
  - Status epilepticus
  - Severe trauma
  - Chest pain with ongoing ischemic changes
  - Acute respiratory distress
  - Uncontrolled hemorrhage
  - Anaphylaxis
  - Acute myocardial infarction with cardiogenic shock

**Assignment:** → **ESI-1**

---

### Rule 2: Does patient require high-risk situation assessment?

**ESI-2 Criteria:**
- Patients who do NOT require immediate resuscitation BUT present with:
  
#### 2a. High-Risk Situations:
- **Acute coronary syndrome:**
  - Chest pain or equivalent
  - EKG changes
  - Positive troponin
  
- **Severe respiratory distress:**
  - Severe asthma/COPD exacerbation
  - Stridor
  - Severe hypoxemia
  
- **Altered mental status:**
  - Confusion, disorientation
  - Loss of consciousness
  - Drug overdose/poisoning
  
- **Severe infection/sepsis:**
  - Fever + altered mental status
  - Meningitis signs
  - Septic shock (even if stabilized)
  
- **Abdominal pain (potential emergency):**
  - Peritoneal signs
  - Severe abdominal pain
  - Suspected rupture/perforation
  
- **Injury patterns:**
  - Major trauma mechanism
  - Penetrating wounds
  - Significant burns
  
- **Psychiatric emergency:**
  - Suicidal/homicidal ideation with plan
  - Severe agitation

**Assignment:** → **ESI-2**

---

### Rule 3: Does patient require immediate evaluation?

**ESI-3 Criteria:**
- Stable patient (no danger, no high-risk situation)
- BUT has:
  - Acute illness requiring immediate physician assessment
  - Examples:
    - Fever (any age with acute illness)
    - Acute vomiting/diarrhea
    - Moderate pain
    - Significant injury (but not severe trauma)
    - Psychiatric symptoms (stable, not dangerous)

**Assignment:** → **ESI-3**

---

### Rule 4: Does patient require multiple resources?

**ESI-4 Criteria:**
- Stable, no high-risk situation
- Unlikely to require immediate physician evaluation
- BUT likely to require:
  - Laboratory tests (blood work)
  - Imaging (X-ray, CT, ultrasound)
  - Procedures (laceration repair, splinting)
  - Multiple medications
  
Examples:
- Simple laceration (needs repair)
- Ankle sprain (needs X-ray)
- Earache (likely needs exam + possibly imaging)
- Mild dehydration (needs labs + IV)

**Assignment:** → **ESI-4**

---

### Rule 5: Single Resource or No Resources?

**ESI-5 Criteria:**
- Stable patient
- Requires only ONE resource OR no resources
  
**Single Resource Examples:**
- Simple dressing change
- Single lab test only (no imaging, no meds, no procedures)
- Flu shot administration
- Prescription refill

**No Resource Examples:**
- Patient education only
- Reassurance/counseling
- Clearance for return to work/sports

**Assignment:** → **ESI-5**

---

## Clinical Examples

### Example 1: Chest Pain with EKG Changes
- **Question 1:** Immediate danger? NO
- **Question 2:** High-risk situation (ACS with EKG changes)? YES
- **Assignment:** **ESI-2** ✓

### Example 2: Fever, Alert & Oriented
- **Question 1:** Immediate danger? NO
- **Question 2:** High-risk situation? NO (stable vitals, alert)
- **Question 3:** Needs immediate evaluation (fever + acute illness)? YES
- **Assignment:** **ESI-3** ✓

### Example 3: Simple Laceration
- **Question 1-3:** NO → ESI-3 range
- **Question 4:** Multiple resources (repair, dressing, wound care)? YES
- **Assignment:** **ESI-4** ✓

### Example 4: Request for Prescription Refill
- **Question 1-3:** NO
- **Question 4:** Multiple resources? NO
- **Question 5:** Single resource or none? Just prescription = 1 resource
- **Assignment:** **ESI-4** or **ESI-5** (depends on policy)

### Example 5: Status Epilepticus
- **Question 1:** Requires immediate life-saving intervention? YES
- **Assignment:** **ESI-1** ✓

---

## Validation Parameters

### Triage Nurse Assessment Inputs

**Required:**
- Chief complaint
- Vital signs (temperature, heart rate, BP, RR, O₂ saturation)
- Mental status (alert & oriented?)
- Pain level (0-10 scale)

**Optional but Helpful:**
- Injury mechanism
- Previous medical history
- Current medications
- Allergies

---

## Implementation for Agents

### Input Schema

```json
{
  "patient_data": {
    "chief_complaint": "string",
    "vital_signs": {
      "temperature_c": "number",
      "heart_rate": "number (bpm)",
      "blood_pressure": "string (systolic/diastolic)",
      "respiratory_rate": "number",
      "oxygen_saturation": "number (0-100%)"
    },
    "mental_status": "alert|confused|altered|unconscious",
    "pain_level": "number (0-10)",
    "injury_mechanism": "string (optional)",
    "high_risk_flags": [
      "chest_pain",
      "respiratory_distress",
      "altered_mental_status",
      "severe_infection",
      "trauma",
      "psychiatric_emergency"
    ]
  }
}
```

### Output Schema

```json
{
  "esi_level": 1|2|3|4|5,
  "esi_level_name": "string",
  "decision_rule": "Rule 1|2|3|4|5",
  "reasoning": "string",
  "recommendations": {
    "response_time_minutes": "number",
    "immediate_actions": ["string"],
    "physician_notification": "boolean"
  },
  "confidence": "number (0-100%)"
}
```

### Example Query

**Input:**
```json
{
  "chief_complaint": "Chest pain",
  "vital_signs": {
    "temperature_c": 36.5,
    "heart_rate": 95,
    "blood_pressure": "145/92",
    "respiratory_rate": 18,
    "oxygen_saturation": 98
  },
  "mental_status": "alert",
  "pain_level": 7,
  "high_risk_flags": ["chest_pain"]
}
```

**Output:**
```json
{
  "esi_level": 2,
  "esi_level_name": "Emergency",
  "decision_rule": "Rule 2a - High-Risk Situation (Acute Coronary Syndrome)",
  "reasoning": "Patient presents with chest pain, elevated BP/HR, which suggests acute coronary syndrome. Requires immediate physician evaluation and EKG.",
  "recommendations": {
    "response_time_minutes": 10,
    "immediate_actions": [
      "Place on continuous cardiac monitoring",
      "Obtain 12-lead EKG",
      "Establish IV access",
      "Alert cardiologist on standby"
    ],
    "physician_notification": true
  },
  "confidence": 95
}
```

---

## Common Triage Errors & Solutions

### Error 1: Under-triage (Assigning too low)
**Problem:** ESI-4 assigned to patient with acute stroke
**Solution:** Remember Rule 2a - stroke (potential thrombolytic) = ESI-2

### Error 2: Over-triage (Assigning too high)
**Problem:** ESI-2 assigned to patient with simple ankle sprain
**Solution:** Rule 4 - needs imaging only = ESI-4, not ESI-2

### Error 3: Confusion with "Resources"
**Problem:** Counting physician visit as a "resource"
**Solution:** ALL ED patients see physician. Count diagnostic/therapeutic resources only

### Error 4: Missing high-risk situations
**Problem:** Missing cases with altered mental status
**Solution:** Any acute mental status change = Rule 2b = ESI-2

---

## Reliability & Validity

**Sensitivity:** 95-97%  
**Specificity:** 85-90%  
**Inter-rater Reliability:** Kappa 0.80-0.85  
**Outcome Prediction:** Strong correlation with adverse outcomes

---

## References

- Gilboy, N., Tanabe, P., Travers, D., & Rosenau, A. M. (2012). *Emergency Severity Index (ESI): A triage tool for emergency department care* (5th ed.). Institute for Safe Medication Practices.
- American College of Emergency Physicians (ACEP) - ESI Implementation Resources
- Society for Academic Emergency Medicine (SAEM) - Triage Guidelines

---

## Related Tools

- MANCHESTER TRIAGE SYSTEM (MTS) - Alternative 5-level triage
- CEDOCS - Canadian ED Triage
- AUSTRALASIAN TRIAGE SCALE (ATS) - 5-level Australian system
- RAPID EMERGENCY TRIAGE AND TREATMENT SYSTEM (RETTS) - Australian alternative

---

## Clinical Notes for Agents

When consulting ESI algorithm, remember:

1. **Safety First:** ESI-1 and ESI-2 patients take priority
2. **High-Risk Trumps Vital Signs:** Patient with normal vitals but chest pain = ESI-2
3. **Reassess:** ESI level can change as patient condition evolves
4. **Communication:** Always communicate rationale to clinical team
5. **Limitations:** ESI is initial triage tool; not diagnostic

---

**Last Updated:** 2026-05-15  
**Standard:** ESI-4 (2012)  
**Status:** Ready for Clinical Use  
**Review Cycle:** Annual
