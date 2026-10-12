"""Deploy the games Worker to Cloudflare.

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
        ],
    }
    script = (HERE / "index.js").read_text()
    files = {
        "metadata": (None, json.dumps(metadata), "application/json"),
        "index.js": ("index.js", script, "application/javascript+module"),
    }
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


if __name__ == "__main__":
    main()
