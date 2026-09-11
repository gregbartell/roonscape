"""Throwaway palette comparison. Run: python src/renderer/palette-prototype/serve.py"""
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from functools import partial

root = Path(__file__).resolve().parent
print("Palette prototype: http://localhost:8765/?variant=A", flush=True)
ThreadingHTTPServer(("127.0.0.1", 8765), partial(SimpleHTTPRequestHandler, directory=str(root))).serve_forever()
