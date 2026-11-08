# This file is part of Stratagram: https://github.com/shinsterneck/stratagram
# Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
# SPDX-License-Identifier: Apache-2.0

# A smoke test of the plugin in a running NetBox with site/netbox-seed.py
# loaded (the site Vienna DC): the Diagram tabs of the site and its
# locations, NetBox's data for the editor, saving a diagram and reading it
# back, and saving refused to a user without the permission. Run inside
# NetBox:
#   manage.py shell -c "exec(open('smoke.py').read())"
# It prints "smoke test passed", or fails with what went wrong.
import json

from django.contrib.auth import get_user_model
from django.test import Client
from django.urls import reverse

from dcim.models import Location, Site
from netbox_stratagram.models import SiteDiagram


def check(ok, what):
    if not ok:
        raise SystemExit(f"smoke test failed: {what}")


site = Site.objects.get(slug="vienna-dc")
User = get_user_model()
admin = User.objects.filter(is_superuser=True).first()
check(admin is not None, "no superuser to log in with")
client = Client()
client.force_login(admin)


def get(url):
    return client.get(url, HTTP_HOST="localhost")


# The Diagram tabs: the site's, and each location's.
r = get(f"/dcim/sites/{site.pk}/diagram/")
check(r.status_code == 200, f"site tab: {r.status_code}")
check(b"stratagram" in r.content, "site tab: no editor on the page")
locations = list(Location.objects.filter(site=site))
check(len(locations) == 2, f"expected 2 locations, got {len(locations)}")
for loc in locations:
    r = get(f"/dcim/locations/{loc.pk}/diagram/")
    check(r.status_code == 200, f"location {loc.name} tab: {r.status_code}")

# NetBox's data for the editor.
r = get(reverse("plugins:netbox_stratagram:site_netbox_data", kwargs={"pk": site.pk}))
check(r.status_code == 200, f"site data: {r.status_code}")
data = r.json()
devices = data["devices"]["results"] if isinstance(data["devices"], dict) else data["devices"]
check(len(devices) == 10, f"expected 10 devices, got {len(devices)}")
check(len(data["cables"]) == 16, f"expected 16 cables, got {len(data['cables'])}")
check(len(data["locations"]) == 2, "expected the 2 locations")
switch = next(d for d in devices if d["name"] == "access-01")
check((switch.get("switch_ports") or {}).get("count") == 48, "access-01 should have 48 copper ports")

# Saving a diagram and reading it back.
url = reverse("plugins:netbox_stratagram:site_diagram_data", kwargs={"pk": site.pk})
SiteDiagram.objects.filter(site=site).delete()
diagram = {"format": "stratagram", "version": 1, "name": "Vienna DC", "devices": [], "links": []}
r = client.post(url, json.dumps({"diagram": diagram}), content_type="application/json", HTTP_HOST="localhost")
check(r.status_code == 200 and "saved" in r.json(), f"save: {r.status_code} {r.content[:200]!r}")
r = get(url)
check(r.status_code == 200 and r.json()["diagram"]["name"] == "Vienna DC", "saved diagram not read back")

# Without the permission to change site diagrams: refused.
plain, _ = User.objects.get_or_create(username="smoke-viewer")
other = Client()
other.force_login(plain)
r = other.post(url, json.dumps({"diagram": diagram}), content_type="application/json", HTTP_HOST="localhost")
check(r.status_code in (403, 404), f"save without permission: {r.status_code}")
SiteDiagram.objects.filter(site=site).delete()

print("smoke test passed")
