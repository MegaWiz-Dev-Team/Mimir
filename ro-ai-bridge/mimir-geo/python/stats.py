#!/usr/bin/env python3
"""Stateless spatial-statistics kernel — invoked by Rust (`stats_py.rs`) as:

    python stats.py <request.json> <response.json>

Reads a request {method, ...payload}, computes, writes {ok, result|error}. NO
network, NO state between calls; resource caps + timeout enforced by the Rust
parent. Methods: moran | lisa | kriging | pointpattern.

P4 TODO: implement each method against esda/libpysal/pointpats/scipy. Keep output
JSON-serialisable (floats/lists). Never echo back raw input rows in errors.
"""
import json
import sys


def moran(payload):       raise NotImplementedError("P4: esda.Moran on values + libpysal weights")
def lisa(payload):        raise NotImplementedError("P4: esda.Moran_Local (local indicators)")
def kriging(payload):     raise NotImplementedError("P4: scipy ordinary kriging interpolation")
def pointpattern(payload): raise NotImplementedError("P4: pointpats Ripley's K / NN")

METHODS = {"moran": moran, "lisa": lisa, "kriging": kriging, "pointpattern": pointpattern}


def main():
    req = json.loads(open(sys.argv[1]).read())
    method = req.pop("method", None)
    try:
        if method not in METHODS:
            raise ValueError(f"unknown method: {method}")
        out = {"ok": True, "result": METHODS[method](req)}
    except Exception as e:  # noqa: BLE001 — return as JSON, never leak a traceback w/ data
        out = {"ok": False, "error": f"{type(e).__name__}: {e}"}
    open(sys.argv[2], "w").write(json.dumps(out))


if __name__ == "__main__":
    main()
