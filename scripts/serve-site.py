#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///

import argparse
import functools
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser(description="Serve the looprs static site")
    parser.add_argument("site_dir", type=Path)
    parser.add_argument("port", type=int)
    args = parser.parse_args()

    handler = functools.partial(SimpleHTTPRequestHandler, directory=str(args.site_dir))
    server = ThreadingHTTPServer(("127.0.0.1", args.port), handler)
    actual_port = server.server_address[1]
    print(f"Serving looprs static site from: {args.site_dir}", flush=True)
    print(f"URL: http://127.0.0.1:{actual_port}", flush=True)
    server.serve_forever()


if __name__ == "__main__":
    main()
