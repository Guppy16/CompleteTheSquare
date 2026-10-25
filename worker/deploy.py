"""Deploy the games Worker (its MCP connector and online games) to Cloudflare.

    set -a && . ./.env && python3 worker/deploy.py

The token in CLOUDFLARE_API_TOKEN needs Workers Scripts edit and D1 edit.
Safe to re-run: the database and its tables are only created once.

A test copy, with its own database and Durable Objects, for trying a change
before it goes live (the page on localhost talks to it):

    set -a && . ./.env && CTS_WORKER=cts-dev CTS_DATABASE=cts-dev-games python3 worker/deploy.py
"""

import json
import os
from pathlib import Path

import requests

API = "https://api.cloudflare.com/client/v4"
# The Worker serves https://cts.<account subdomain>.workers.dev; the database keeps its first name.
NAME = os.environ.get("CTS_WORKER", "cts")
DATABASE = os.environ.get("CTS_DATABASE", "cts-games")
HERE = Path(__file__).parent

# The rate limiter's period is 10 or 60 seconds: two a minute is about one per 30 s.
RATE_LIMIT = {"limit": 2, "period": 60}
# Online games: creating one, joining the queue, (re)connecting, per IP.
ONLINE_RATE_LIMIT = {"limit": 30, "period": 60}

# Durable Object classes, created by migrations applied in order, each once:
# v1 the Game class (worker/game.js), one per game against Claude; v2 the
# Room class, one per online game, and the Lobby (worker/online.js).
MIGRATIONS = [
    ("v1", {"new_sqlite_classes": ["Game"]}),
    ("v2", {"new_sqlite_classes": ["Room", "Lobby"]}),
]


def call(method: str, path: str, **kwargs) -> dict | list:
    """Call the Cloudflare API and return its result; raise on an error reply."""
    headers = {"Authorization": f"Bearer {os.environ['CLOUDFLARE_API_TOKEN']}"}
    response = requests.request(method, API + path, headers=headers, **kwargs)
    response.raise_for_status()
    return response.json()["result"]


def database_id(account: str) -> str:
    """The D1 database's id, creating the database if it does not exist."""
    found = call("GET", f"/accounts/{account}/d1/database", params={"name": DATABASE})
    if found:
        return found[0]["uuid"]
    created = call("POST", f"/accounts/{account}/d1/database", json={"name": DATABASE})
    return created["uuid"]


def migration_tag(account: str) -> str | None:
    """The tag of the last Durable Object migration applied to the Worker."""
    scripts = call("GET", f"/accounts/{account}/workers/scripts")
    return next((s.get("migration_tag") for s in scripts if s["id"] == NAME), None)


def upload_worker(account: str, database: str) -> None:
    metadata = {
        "main_module": "index.js",
        "compatibility_date": "2025-09-01",
        "bindings": [
            {"type": "d1", "name": "DB", "id": database},
            {
                "type": "ratelimit",
                "name": "LIMITER",
                "namespace_id": "1001",
                "simple": RATE_LIMIT,
            },
            {
                "type": "ratelimit",
                "name": "ONLINE_LIMITER",
                "namespace_id": "1002",
                "simple": ONLINE_RATE_LIMIT,
            },
            {"type": "durable_object_namespace", "name": "GAMES", "class_name": "Game"},
            {"type": "durable_object_namespace", "name": "ROOMS", "class_name": "Room"},
            {"type": "durable_object_namespace", "name": "LOBBY", "class_name": "Lobby"},
        ],
    }
    # The migrations after the last one applied, if any.
    tag = migration_tag(account)
    tags = [t for t, _ in MIGRATIONS]
    pending = MIGRATIONS[tags.index(tag) + 1:] if tag else MIGRATIONS
    if pending:
        metadata["migrations"] = {
            **({"old_tag": tag} if tag else {}),
            "new_tag": pending[-1][0],
            "steps": [step for _, step in pending],
        }
    modules = {
        "index.js": (HERE / "index.js", "application/javascript+module"),
        "mcp.js": (HERE / "mcp.js", "application/javascript+module"),
        "game.js": (HERE / "game.js", "application/javascript+module"),
        "rules.js": (HERE / "rules.js", "application/javascript+module"),
        "online.js": (HERE / "online.js", "application/javascript+module"),
        "ratings.js": (HERE / "ratings.js", "application/javascript+module"),
        "board.html": (HERE / "board.html", "text/plain"),
        "icon.svg": (HERE.parent / "favicon.svg", "text/plain"),
        "icon.png": (HERE / "icon.png", "application/octet-stream"),
        "ai.wasm": (HERE.parent / "square-game" / "ai.wasm", "application/wasm"),
    }
    files = {"metadata": (None, json.dumps(metadata), "application/json")}
    for name, (path, content_type) in modules.items():
        files[name] = (name, path.read_bytes(), content_type)
    call("PUT", f"/accounts/{account}/workers/scripts/{NAME}", files=files)


def main() -> None:
    account = call("GET", "/accounts")[0]["id"]
    database = database_id(account)
    schema = (HERE / "schema.sql").read_text()
    query = f"/accounts/{account}/d1/database/{database}/query"
    call("POST", query, json={"sql": schema})

    upload_worker(account, database)
    script = f"/accounts/{account}/workers/scripts/{NAME}"
    call("POST", f"{script}/subdomain", json={"enabled": True})

    subdomain = call("GET", f"/accounts/{account}/workers/subdomain")["subdomain"]
    print(f"https://{NAME}.{subdomain}.workers.dev/games")
    print(f"https://{NAME}.{subdomain}.workers.dev/mcp")


if __name__ == "__main__":
    main()
