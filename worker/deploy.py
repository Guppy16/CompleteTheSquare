"""Deploy the games Worker (and its MCP connector) to Cloudflare.

    set -a && . ./.env && python3 worker/deploy.py

The token in CLOUDFLARE_API_TOKEN needs Workers Scripts edit and D1 edit.
Safe to re-run: the database and its table are only created once.
"""

import json
import os
from pathlib import Path

import requests

API = "https://api.cloudflare.com/client/v4"
NAME = "cts-games"
HERE = Path(__file__).parent

# The rate limiter's period is 10 or 60 seconds: two a minute is about one per 30 s.
RATE_LIMIT = {"limit": 2, "period": 60}

# Creates the Game class (worker/game.js), one Durable Object per game against Claude.
MIGRATION = {"new_tag": "v1", "new_sqlite_classes": ["Game"]}


def call(method: str, path: str, **kwargs) -> dict | list:
    """Call the Cloudflare API and return its result; raise on an error reply."""
    headers = {"Authorization": f"Bearer {os.environ['CLOUDFLARE_API_TOKEN']}"}
    response = requests.request(method, API + path, headers=headers, **kwargs)
    response.raise_for_status()
    return response.json()["result"]


def database_id(account: str) -> str:
    """The D1 database's id, creating the database if it does not exist."""
    found = call("GET", f"/accounts/{account}/d1/database", params={"name": NAME})
    if found:
        return found[0]["uuid"]
    created = call("POST", f"/accounts/{account}/d1/database", json={"name": NAME})
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
            {"type": "durable_object_namespace", "name": "GAMES", "class_name": "Game"},
        ],
    }
    # Durable Object classes are created by a migration, applied once.
    tag = migration_tag(account)
    if tag != MIGRATION["new_tag"]:
        metadata["migrations"] = {"old_tag": tag, **MIGRATION} if tag else MIGRATION
    modules = {
        "index.js": (HERE / "index.js", "application/javascript+module"),
        "mcp.js": (HERE / "mcp.js", "application/javascript+module"),
        "game.js": (HERE / "game.js", "application/javascript+module"),
        "rules.js": (HERE / "rules.js", "application/javascript+module"),
        "board.html": (HERE / "board.html", "text/plain"),
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
