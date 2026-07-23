#!/usr/bin/env python3
"""Build brand -> ingredient TSV from RxNav (RxNorm, public domain).
Dev-time batch job. Writes incrementally so partial progress survives.
Ships in product as a static table (no runtime external calls)."""
import json, urllib.request, urllib.parse, sys

BRANDS = """tylenol advil motrin aleve coumadin plavix lipitor crestor zocor nexium prilosec
prevacid glucophage januvia lantus ventolin proair advair singulair zyrtec claritin allegra
benadryl prozac zoloft lexapro effexor wellbutrin xanax ativan ambien norvasc toprol cozaar
diovan lasix coreg zithromax cipro levaquin augmentin keflex bactrim flagyl viagra cialis
synthroid medrol neurontin lyrica percocet vicodin morphine eliquis xarelto pradaxa flonase
glucotrol amaryl protonix""".split()

def get(url):
    with urllib.request.urlopen(url, timeout=8) as r:
        return json.load(r)

def rxcui(name):
    u = "https://rxnav.nlm.nih.gov/REST/rxcui.json?" + urllib.parse.urlencode({"name": name, "search": 2})
    ids = (get(u).get("idGroup") or {}).get("rxnormId") or []
    return ids[0] if ids else None

def ingredients(cui):
    d = get(f"https://rxnav.nlm.nih.gov/REST/rxcui/{cui}/related.json?tty=IN")
    groups = (d.get("relatedGroup") or {}).get("conceptGroup") or []
    return [p["name"].lower() for g in groups if g.get("tty") == "IN"
            for p in g.get("conceptProperties", [])]

out = open("data/rxnorm_brand_ingredient.tsv", "w")
out.write("# brand<TAB>ingredient(s) — built dev-time from RxNav (RxNorm, public domain).\n")
n = 0
for b in BRANDS:
    try:
        cui = rxcui(b)
        ings = ingredients(cui) if cui else []
        if ings:
            out.write(f"{b}\t{';'.join(ings)}\n"); out.flush(); n += 1
            print(f"  {b:12} -> {';'.join(ings)}", flush=True)
        else:
            print(f"  {b:12} -> (miss)", flush=True)
    except Exception as e:
        print(f"  {b:12} -> ERR {e}", flush=True)
out.close()
print(f"DONE: {n}/{len(BRANDS)} resolved", flush=True)
