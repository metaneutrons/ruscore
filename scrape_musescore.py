#!/opt/homebrew/opt/python@3.11/bin/python3.11
"""
Scrape MuseScore score SVGs and build a PDF.

Step 1: Close Chrome completely, then run this script.
        It launches a REAL Chrome with remote debugging — no automation flags.
Step 2: The script connects via CDP, navigates, scrolls, captures SVGs.

Usage:
    python scrape_musescore.py <musescore_url> [output.pdf]
"""

import sys
import re
import time
import subprocess
from pathlib import Path
from playwright.sync_api import sync_playwright

CHROME_BIN = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
DEBUG_PORT = 9222


def page_num(url):
    m = re.search(r"score_(\d+)\.svg", url)
    return int(m.group(1)) if m else 0


def scrape_score(url: str, output_pdf: str = "score.pdf"):
    # Launch real Chrome with remote debugging — completely undetectable
    profile = Path("chrome_debug_profile").resolve()
    profile.mkdir(exist_ok=True)

    print("Launching Chrome with remote debugging...")
    chrome = subprocess.Popen([
        CHROME_BIN,
        f"--remote-debugging-port={DEBUG_PORT}",
        f"--user-data-dir={profile}",
        "--no-first-run",
        "--no-default-browser-check",
        "about:blank",
    ])
    time.sleep(3)

    try:
        with sync_playwright() as p:
            # Connect to the real Chrome — no automation flags injected
            browser = p.chromium.connect_over_cdp(f"http://127.0.0.1:{DEBUG_PORT}")
            context = browser.contexts[0]
            page = context.new_page()

            svg_responses = {}

            def handle_response(response):
                if re.search(r"score_\d+\.svg", response.url) and response.ok:
                    try:
                        svg_responses[response.url] = response.body()
                        print(f"  [captured] page {page_num(response.url)} ({len(svg_responses)} total)")
                    except Exception:
                        pass

            page.on("response", handle_response)

            print(f"Navigating to {url}")
            page.goto(url, wait_until="domcontentloaded", timeout=60000)

            print("Waiting for score to appear...")
            page.wait_for_selector('img[src*="score_"]', timeout=120000)
            time.sleep(2)

            # Get total pages
            alt = page.get_attribute('img[src*="score_"][src*=".svg"]', "alt") or ""
            match = re.search(r"(\d+)\s+of\s+(\d+)\s+pages", alt)
            total_pages = int(match.group(2)) if match else 1
            print(f"Score has {total_pages} pages. Captured: {len(svg_responses)}")

            # Scroll inside the score viewer container
            print("Scrolling score viewer...")
            result = page.evaluate("""async () => {
                let el = document.querySelector('img[src*="score_0.svg"]');
                let scrollable = null;
                while (el && el !== document.body) {
                    if (el.scrollHeight > el.clientHeight + 10) { scrollable = el; break; }
                    el = el.parentElement;
                }
                if (!scrollable) return 'no scrollable container';

                for (let pos = 0; pos < scrollable.scrollHeight; pos += 300) {
                    scrollable.scrollTop = pos;
                    await new Promise(r => setTimeout(r, 300));
                }
                scrollable.scrollTop = scrollable.scrollHeight;
                return 'scrolled to ' + scrollable.scrollHeight;
            }""")
            print(f"  {result}")

            time.sleep(3)
            print(f"Captured {len(svg_responses)}/{total_pages} SVGs.")

            browser.close()
    finally:
        chrome.terminate()

    if not svg_responses:
        print("ERROR: No SVGs captured.")
        return

    # Sort and save
    sorted_items = sorted(svg_responses.items(), key=lambda x: page_num(x[0]))
    svg_dir = Path("score_svgs")
    svg_dir.mkdir(exist_ok=True)
    svg_files = []
    for i, (_, data) in enumerate(sorted_items):
        path = svg_dir / f"score_{i}.svg"
        path.write_bytes(data) if isinstance(data, bytes) else path.write_text(data)
        svg_files.append(path)

    # Convert to PDF
    print(f"Converting {len(svg_files)} SVGs to PDF...")
    import cairosvg
    from pypdf import PdfReader, PdfWriter
    import io

    writer = PdfWriter()
    for svg_path in svg_files:
        pdf_data = cairosvg.svg2pdf(file_obj=open(svg_path, "rb"))
        for pg in PdfReader(io.BytesIO(pdf_data)).pages:
            writer.add_page(pg)

    with open(output_pdf, "wb") as f:
        writer.write(f)

    print(f"Done! {output_pdf} ({len(svg_files)} pages)")


if __name__ == "__main__":
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <musescore_url> [output.pdf]")
        sys.exit(1)
    scrape_score(sys.argv[1], sys.argv[2] if len(sys.argv) > 2 else "score.pdf")
