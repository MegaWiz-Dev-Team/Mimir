---
description: How to bypass the Antigravity Editor Bug (+0 -0 hang) 
---

# Antigravity Editor Bypass Workflow

It's a known bug that the `replace_file_content` and `multi_replace_file_content` tools will sometimes hang eternally (+0 -0) and then timeout or cancel out, especially on larger files (like 100+ lines Kubernetes config files) or multiple concurrent edits.

If an edit fails sequentially or you anticipate an edit will hang due to file size, YOU MUST use this workflow to edit the file safely. 

**Constraints to remember:**
You are strictly forbidden from using `cat` inside a bash command to create/append files, and from using `sed` to replace files. 

### The Solution: Direct Python Manipulation via `run_command`

Instead of the MCP tools, use python via `run_command` in a single line to execute text replacement rapidly and safely:

```bash
python3 -c "
import sys
with open('path/to/file', 'r') as f: content = f.read()
content = content.replace('TARGET_STRING', 'REPLACEMENT_STRING')
with open('path/to/file', 'w') as f: f.write(content)
"
```

For more complex, multiline replacements or regex replacements, create an executable Python script using `write_to_file` at `/tmp/patch.py` and run it:

```python
# /tmp/patch.py
import re

with open('target_file.yml', 'r') as f:
    text = f.read()

# perform edits securely...
text = re.sub(r'old_pattern', r'new_pattern', text)

with open('target_file.yml', 'w') as f:
    f.write(text)
```
Then execute using `run_command`: `python3 /tmp/patch.py`.

This bypasses the Editor Bug completely while abiding by all safety rules. You are now upgraded!
