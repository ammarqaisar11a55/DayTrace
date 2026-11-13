"""Uploads the Snap Store listing's icon and screenshots.

The snapcraft command can push an icon only together with the listing text,
and cannot push screenshots, so this talks to the Store's binary metadata API
directly. It authenticates with SNAPCRAFT_STORE_CREDENTIALS, as exported by
`snapcraft export-login`, and never prints it.

    python3 store_listing.py --check               # show the current media only
    python3 store_listing.py icon.png shot1.png ...  # replace icon and screenshots
"""
import argparse
import base64
import hashlib
import json
import os
import sys
from pathlib import Path

import requests
from pymacaroons import Macaroon

SNAP = "daytrace"
DASHBOARD = "https://dashboard.snapcraft.io/dev/api"
# Media the Store accepts on a listing page.
MAX_SCREENSHOTS = 5


def credentials() -> tuple:
    encoded = os.environ.get("SNAPCRAFT_STORE_CREDENTIALS", "")
    if not encoded:
        sys.exit("SNAPCRAFT_STORE_CREDENTIALS is not set")
    try:
        exported = json.loads(base64.b64decode(encoded))
    except ValueError:
        sys.exit("SNAPCRAFT_STORE_CREDENTIALS is not an exported snapcraft login")
    if exported.get("t") != "u1-macaroon":
        sys.exit(f"unsupported credential type: {exported.get('t')!r}")
    return exported["v"]["r"], exported["v"]["d"]


def refresh(discharge: str) -> str:
    """Ubuntu One renews a discharge the Store considers too old, as snapcraft does."""
    response = requests.post(
        "https://login.ubuntu.com/api/v2/tokens/refresh",
        json={"discharge_macaroon": discharge},
        headers={"Accept": "application/json"},
        timeout=30,
    )
    if response.status_code != 200:
        sys.exit(f"refreshing the login failed: {response.status_code}")
    return response.json()["discharge_macaroon"]


def authorization(root: str, discharge: str) -> str:
    bound = Macaroon.deserialize(root).prepare_for_request(Macaroon.deserialize(discharge))
    return f'Macaroon root="{root}", discharge="{bound.serialize()}"'


def needs_refresh(response: requests.Response) -> bool:
    if response.status_code != 401:
        return False
    try:
        errors = response.json().get("error_list", [])
    except ValueError:
        return False
    return any(error.get("code") == "macaroon-needs-refresh" for error in errors)


def snap_id() -> str:
    response = requests.get(
        f"https://api.snapcraft.io/v2/snaps/info/{SNAP}",
        headers={"Snap-Device-Series": "16"},
        timeout=30,
    )
    response.raise_for_status()
    return response.json()["snap-id"]


def describe(media: list) -> str:
    return "\n".join(f"  {item['type']}: {item.get('filename')} {item.get('hash', '')[:12]}" for item in media)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="only show the current media")
    parser.add_argument("icon", nargs="?", type=Path)
    parser.add_argument("screenshots", nargs="*", type=Path)
    arguments = parser.parse_args()

    url = f"{DASHBOARD}/snaps/{snap_id()}/binary-metadata"
    root, discharge = credentials()
    headers = {"Authorization": authorization(root, discharge), "Accept": "application/json"}

    current = requests.get(url, headers=headers, timeout=30)
    if needs_refresh(current):
        headers["Authorization"] = authorization(root, refresh(discharge))
        current = requests.get(url, headers=headers, timeout=30)
    if current.status_code != 200:
        sys.exit(f"reading the listing failed: {current.status_code} {current.text[:300]}")
    print("current media:\n" + describe(current.json()))
    if arguments.check:
        return

    if not arguments.icon or not arguments.screenshots:
        sys.exit("give an icon and at least one screenshot")
    if len(arguments.screenshots) > MAX_SCREENSHOTS:
        sys.exit(f"the Store shows at most {MAX_SCREENSHOTS} screenshots")

    # Media of other kinds, such as a banner, is kept as it is.
    info = [item for item in current.json() if item["type"] not in ("icon", "screenshot")]
    files = []
    uploads = [("icon", "icon", arguments.icon)] + [
        ("screenshot", f"screenshot{index}", path) for index, path in enumerate(arguments.screenshots, 1)
    ]
    for kind, key, path in uploads:
        data = path.read_bytes()
        info.append({"type": kind, "key": key, "filename": path.name, "hash": hashlib.sha256(data).hexdigest()})
        files.append((key, (path.name, data, "image/png")))

    response = requests.put(url, headers=headers, data={"info": json.dumps(info)}, files=files, timeout=120)
    if response.status_code != 200:
        sys.exit(f"uploading failed: {response.status_code} {response.text[:500]}")
    print("uploaded media:\n" + describe(response.json()))


if __name__ == "__main__":
    main()
