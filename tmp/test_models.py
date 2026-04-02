import urllib.request
import json
import os

req = urllib.request.Request("http://localhost:30000/api/v1/models")
try:
    with urllib.request.urlopen(req) as resp:
        data = json.loads(resp.read().decode('utf-8'))
        print("Models present in Dashboard backend:")
        for m in data.get('data', []):
            print(" -", m.get('id'), "| provider:", m.get('provider'))
except Exception as e:
    print("Error fetching models:", e)
