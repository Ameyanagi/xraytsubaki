"""Real browser Wasm/asset-loading check; run with Playwright installed."""
import functools
import json
import os
import threading
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

from playwright.sync_api import sync_playwright

root = Path(__file__).resolve().parents[2]
reference = json.loads((root / "js-rexafs/test/ru-reference.json").read_text())
rows = [[float(v) for v in line.split()] for line in (root / "crates/rexafs/tests/testfiles/Ru_QAS.dat").read_text().splitlines() if line.strip() and not line.lstrip().startswith("#")]
handler = functools.partial(SimpleHTTPRequestHandler, directory=str(root))
server = ThreadingHTTPServer(("127.0.0.1", 0), handler)
threading.Thread(target=server.serve_forever, daemon=True).start()
try:
    with sync_playwright() as p:
        channel = os.environ.get("REXAFS_BROWSER_CHANNEL")
        browser = p.chromium.launch(headless=True, **({"channel": channel} if channel else {}))
        page = browser.new_page()
        page.goto(f"http://127.0.0.1:{server.server_port}/js-rexafs/test/browser.html")
        result = page.evaluate("""async ({rows, reference}) => {
          const {default: init, process} = await import('../browser.js');
          const energy = Float64Array.from(rows, row => row[0]);
          const mu = Float64Array.from(rows, row => Math.log(row[1]/row[2]));
          await init(); // exercises the relative Wasm URL and browser fetch
          const out = process(energy, mu);
          if (out.e0 !== reference.e0 || out.k.length !== reference.k_length || out.r.length !== reference.r_length) throw Error('grid mismatch');
          for (const [key, values] of Object.entries(reference.samples)) {
            [0,20,50,100].forEach((index,i) => {
              if (Math.abs(out[key][index]-values[i]) > 1e-7*Math.max(1,Math.abs(values[i]))) throw Error(`${key}[${index}] mismatch`);
            });
          }
          let rejected = false;
          try { process(new Float64Array([2,1,3]), new Float64Array([1])); } catch { rejected = true; }
          if (!rejected) throw Error('invalid arrays accepted');
          return {e0:out.e0, k:out.k.length, r:out.r.length};
        }""", {"rows": rows, "reference": reference})
        print("Browser pipeline passed:", result)
        browser.close()
finally:
    server.shutdown()
    server.server_close()
