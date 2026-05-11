import json, glob, os
from collections import Counter

paths = glob.glob('/home/dev/.claude/projects/**/*.jsonl', recursive=True)
paths.sort(key=lambda p: os.path.getmtime(p), reverse=True)
paths = paths[:50]

bash_counts = Counter()
mcp_counts = Counter()

for path in paths:
    try:
        with open(path) as f:
            for line in f:
                try:
                    obj = json.loads(line)
                    msg = obj.get('message', {})
                    if msg.get('role') == 'assistant':
                        for item in msg.get('content', []):
                            if item.get('type') == 'tool_use':
                                name = item.get('name', '')
                                inp = item.get('input', {})
                                if name == 'Bash':
                                    cmd = inp.get('command', '').strip()
                                    tokens = cmd.split()
                                    i = 0
                                    while i < len(tokens) and ('=' in tokens[i] or tokens[i] in ('sudo', 'timeout')):
                                        i += 1
                                    if i < len(tokens):
                                        first = tokens[i]
                                        rest = tokens[i+1:i+2]
                                        second = rest[0] if rest else ''
                                        skip = second.startswith('-') or second.startswith('/') or second.startswith('.')
                                        key = (first + ' ' + second) if second and not skip else first
                                        bash_counts[key] += 1
                                elif name.startswith('mcp__'):
                                    mcp_counts[name] += 1
                except Exception:
                    pass
    except Exception:
        pass

print('=== TOP BASH COMMANDS ===')
for cmd, count in bash_counts.most_common(30):
    print(f'{count:4d}  {cmd}')

print()
print('=== TOP MCP CALLS ===')
for tool, count in mcp_counts.most_common(20):
    print(f'{count:4d}  {tool}')
