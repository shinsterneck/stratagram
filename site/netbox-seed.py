# This file is part of Stratagram: https://github.com/shinsterneck/stratagram
# Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
# SPDX-License-Identifier: Apache-2.0

# The site Vienna DC (as site/examples/vienna-dc) in a NetBox with the
# plugin, for the screenshots of the NetBox page (see make-shots.sh). Run
# inside NetBox: manage.py shell -c "exec(open('netbox-seed.py').read())".
# Only for a scratch NetBox: it first deletes ALL cables, circuits and IP
# addresses, and the site's devices, locations and saved diagram.
from dcim.models import (Site, Location, Manufacturer, DeviceType, DeviceRole, Device, Interface,
                         FrontPort, RearPort, PortMapping, Cable)
from dcim.choices import InterfaceTypeChoices as IT, PortTypeChoices
from circuits.models import Provider, CircuitType, Circuit, CircuitTermination
from ipam.models import IPAddress
from netbox_stratagram.models import SiteDiagram
from django.utils.text import slugify

site, _ = Site.objects.get_or_create(name="Vienna DC", slug="vienna-dc")
SiteDiagram.objects.filter(site=site).delete()
Cable.objects.all().delete()
Circuit.objects.all().delete()
IPAddress.objects.all().delete()
Device.objects.filter(site=site).delete()
Location.objects.filter(site=site).delete()
site.description = "Data center, two rooms"
site.save()

r1 = Location.objects.create(site=site, name="Room 1 (core)", slug="room-1")
r2 = Location.objects.create(site=site, name="Room 2 (servers)", slug="room-2")
mf, _ = Manufacturer.objects.get_or_create(name="Generic", slug="generic")

def role(name, color):
    return DeviceRole.objects.get_or_create(name=name, slug=slugify(name), defaults={"color": color})[0]

def device(name, r, model, loc):
    t, _ = DeviceType.objects.get_or_create(manufacturer=mf, model=model, slug=slugify(model))
    return Device.objects.create(name=name, site=site, location=loc, role=r, device_type=t)

def ifs(d, names, kind, mode=None):
    out = {}
    for n in names:
        out[n] = Interface.objects.create(device=d, name=n, type=kind, mode=mode)
    return out

ips = iter(range(1, 250))
def ip(i):
    IPAddress.objects.create(address=f"10.10.0.{next(ips)}/24", assigned_object=i)

def cable(a, b, label=""):
    c = Cable(a_terminations=[a], b_terminations=[b], label=label)
    c.full_clean()
    c.save()

fw_r, sw_r, pp_r, srv_r = role("Firewall", "f44336"), role("Switch", "2196f3"), role("Patch panel", "9e9e9e"), role("Server", "4caf50")
fw = [device(f"fw-0{i}", fw_r, "FW-100", r1) for i in (1, 2)]
core = [device(f"core-0{i}", sw_r, "Core-32", r1) for i in (1, 2)]
acc = device("access-01", sw_r, "Access-48", r2)
pp = device("pp-r2-01", pp_r, "PP-24", r2)
srv = [device(f"srv-0{i}", srv_r, "Server-1U", r2) for i in (1, 2, 3, 4)]

fwi = [ifs(f, ["wan1", "ha", "port3"], IT.TYPE_1GE_FIXED) for f in fw]
cri = [ifs(c, [f"Te1/0/{n}" for n in range(1, 33)], IT.TYPE_10GE_SFP_PLUS) for c in core]
ai = ifs(acc, [f"Gi1/0/{n}" for n in range(1, 49)], IT.TYPE_1GE_FIXED, "access")
ai.update(ifs(acc, [f"Te1/1/{n}" for n in range(1, 5)], IT.TYPE_10GE_SFP_PLUS, "tagged"))
si = [ifs(s, ["eth0"], IT.TYPE_1GE_FIXED)["eth0"] for s in srv]
for c in cri:
    for n in (5, 31):
        c[f"Te1/0/{n}"].mode = "tagged"
        c[f"Te1/0/{n}"].save()
for k in range(2):
    ip(fwi[k]["wan1"]); ip(fwi[k]["port3"]); ip(cri[k]["Te1/0/1"])

fronts, rears = [], []
for n in range(1, 25):
    fp = FrontPort.objects.create(device=pp, name=str(n), type=PortTypeChoices.TYPE_8P8C)
    rp = RearPort.objects.create(device=pp, name=f"R{n}", type=PortTypeChoices.TYPE_8P8C)
    PortMapping(front_port=fp, rear_port=rp, device=pp).save()
    fronts.append(fp)
    rears.append(rp)

ctype, _ = CircuitType.objects.get_or_create(name="Internet", slug="internet")
for k, (prov, cid) in enumerate([("A1", "VIE-INET-1"), ("Magenta", "VIE-INET-2")]):
    p, _ = Provider.objects.get_or_create(name=prov, slug=slugify(prov))
    circ = Circuit.objects.create(cid=cid, provider=p, type=ctype)
    term = CircuitTermination(circuit=circ, term_side="A", termination=site)
    term.save()
    cable(term, fwi[k]["wan1"])

cable(fwi[0]["ha"], fwi[1]["ha"], "HA")
for k in range(2):
    cable(fwi[k]["port3"], cri[k]["Te1/0/1"])
cable(cri[0]["Te1/0/31"], cri[1]["Te1/0/31"], "vPC peer")
cable(cri[0]["Te1/0/5"], ai["Te1/1/1"], "Uplink")
cable(cri[1]["Te1/0/5"], ai["Te1/1/2"])
for k in range(4):
    cable(ai[f"Gi1/0/{k + 1}"], fronts[k])
    cable(rears[k], si[k])
print("seeded", Device.objects.filter(site=site).count(), "devices,", Cable.objects.count(), "cables, site", site.pk)
