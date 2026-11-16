"""A throwaway OpenAI-compatible endpoint, for testing the endpoint lamp and
the streaming transport.

It answers /v1/models with one model and 8192 tokens of context, and
/v1/chat/completions with a short reply. Nothing is stored.

    python tools/mock_endpoint.py                 # plain one-word reply
    python tools/mock_endpoint.py --no-models     # answers, but names no model -> red
    python tools/mock_endpoint.py --unauthorized  # 401 on everything          -> red
    python tools/mock_endpoint.py --port 5099     # any free port

Streaming:

    python tools/mock_endpoint.py --stream                # server-sent events
    python tools/mock_endpoint.py --stream --slow 80      # slower, easier to watch
    python tools/mock_endpoint.py --stream --cut          # first reply hits the limit
    python tools/mock_endpoint.py --stream --error-mid    # fails halfway through
    python tools/mock_endpoint.py --plain                 # ignores stream=true

Point Folio at it under Settings: address http://127.0.0.1:5099/v1, any key.
The lamp then greys out because 127.0.0.1 without a key counts as the demo
writer; type a key, or start Folio with FOLIO_VIEW=probe-live (see README).
"""

import argparse
import json
import re
import time
from http.server import BaseHTTPRequestHandler, HTTPServer

STREAM_TEXT = (
    "The wind changed before Mira reached the door. It came off the water "
    "carrying the smell of cold iron, and the house settled around her as if "
    "it had been waiting. She put one hand on the frame and listened. "
    "Somewhere below, a rope tapped twice against a mast."
)

parser = argparse.ArgumentParser(description="A fake AI endpoint for Folio.")
parser.add_argument("--port", type=int, default=5099)
parser.add_argument(
    "--no-models", action="store_true", help="answer /models with 404 instead of a list"
)
parser.add_argument(
    "--unauthorized", action="store_true", help="answer everything with 401"
)
parser.add_argument(
    "--stream", action="store_true", help="answer chat with server-sent events"
)
parser.add_argument(
    "--slow", type=int, default=25, help="milliseconds between stream chunks"
)
parser.add_argument(
    "--cut",
    action="store_true",
    help="end the first streamed reply at a token limit, so Folio continues it",
)
parser.add_argument(
    "--plain", action="store_true", help="ignore stream=true and answer in one JSON body"
)
parser.add_argument(
    "--error-mid", action="store_true", help="fail halfway through the stream"
)
ARGS = parser.parse_args()

# The first streamed request can be cut off; the continuation gets the rest.
STREAM_CALLS = [0]


class Handler(BaseHTTPRequestHandler):
    def _send(self, status, payload):
        body = json.dumps(payload).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _send_stream(self, chunks):
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.send_header("Connection", "close")
        self.end_headers()
        self.close_connection = True
        for index, chunk in enumerate(chunks):
            if ARGS.error_mid and index == 3:
                payload = {"error": {"message": "mock stream failure"}}
                self.wfile.write(f"data: {json.dumps(payload)}\n\n".encode("utf-8"))
                self.wfile.flush()
                return
            payload = {"choices": [{"delta": {"content": chunk}}]}
            self.wfile.write(f"data: {json.dumps(payload)}\n\n".encode("utf-8"))
            self.wfile.flush()
            if ARGS.slow > 0:
                time.sleep(ARGS.slow / 1000.0)
        reason = "length" if ARGS.cut and STREAM_CALLS[0] == 0 else "stop"
        payload = {"choices": [{"delta": {}, "finish_reason": reason}]}
        self.wfile.write(f"data: {json.dumps(payload)}\n\n".encode("utf-8"))
        self.wfile.write(b"data: [DONE]\n\n")
        self.wfile.flush()

    def do_GET(self):
        if ARGS.unauthorized:
            self._send(401, {"error": "invalid api key"})
            return
        if self.path.rstrip("/").endswith("/models") and not ARGS.no_models:
            self._send(
                200,
                {
                    "object": "list",
                    "data": [
                        {
                            "id": "mock-model",
                            "object": "model",
                            "context_length": 8192,
                        }
                    ],
                },
            )
        else:
            self._send(404, {"error": "not found"})

    def do_POST(self):
        if ARGS.unauthorized:
            self._send(401, {"error": "invalid api key"})
            return
        if not self.path.rstrip("/").endswith("/chat/completions"):
            self._send(404, {"error": "not found"})
            return

        length = int(self.headers.get("Content-Length", 0) or 0)
        raw = self.rfile.read(length) if length else b"{}"
        try:
            request = json.loads(raw or b"{}")
        except ValueError:
            request = {}

        wants_stream = bool(request.get("stream")) and not ARGS.plain
        if not (ARGS.stream and wants_stream):
            self._send(
                200,
                {"choices": [{"message": {"role": "assistant", "content": "ready"}}]},
            )
            return

        chunks = re.findall(r"\S+\s*", STREAM_TEXT)
        if ARGS.cut and STREAM_CALLS[0] == 0:
            chunks = chunks[: max(1, len(chunks) // 3)]
        self._send_stream(chunks)
        STREAM_CALLS[0] += 1

    def log_message(self, *args):
        pass


if __name__ == "__main__":
    print(f"mock endpoint on http://127.0.0.1:{ARGS.port}/v1  (Ctrl+C stops it)")
    HTTPServer(("127.0.0.1", ARGS.port), Handler).serve_forever()
