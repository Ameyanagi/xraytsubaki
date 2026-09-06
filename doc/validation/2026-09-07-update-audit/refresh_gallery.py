"""Build the local screenshot gallery from recorded QA observations."""
import html
import json
from pathlib import Path

root = Path(__file__).resolve().parent
steps = json.loads((root / 'steps.json').read_text())
parts = ['''<!doctype html><meta charset="utf-8"><title>rexafs update-channel audit</title>
<style>body{font:16px system-ui;max-width:1200px;margin:40px auto;padding:0 20px;background:#f5f6fa;color:#202533}article{background:white;padding:20px;margin:24px 0;border-radius:12px}img{width:100%;height:auto}p{line-height:1.5}nav a{display:block;margin:7px 0}</style>
<h1>rexafs update-channel audit · 7 September 2026</h1>
<p>Development builds with the update-channel changes on macOS ARM. Captions distinguish observed failures, corrections and passing checks. See README.md for validation status.</p><nav>''']
for i, step in enumerate(steps, 1):
    parts.append(f'<a href="#step-{i}">{i:02d}. {html.escape(step["file"])}</a>')
parts.append('</nav>')
for i, step in enumerate(steps, 1):
    file = html.escape(step['file'], quote=True)
    note = html.escape(step['note'], quote=True)
    parts.append(f'<article id="step-{i}"><h2>{i:02d}. {file}</h2><p>{note}</p><a href="{file}"><img loading="lazy" src="{file}" alt="{note}"></a></article>')
(root / 'index.html').write_text('\n'.join(parts))
print(f'Gallery: {len(steps)} recorded screenshots')
