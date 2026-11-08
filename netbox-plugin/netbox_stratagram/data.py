# This file is part of Stratagram: https://github.com/shinsterneck/stratagram
# Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
# SPDX-License-Identifier: Apache-2.0

"""A site's devices, cabled interfaces and cables for the diagram, with the
ports of patch panels and switches (and which socket each cable is in).

The shapes are those of NetBox's REST API (the fields the diagram uses),
so the editor reads this the same as API output. Everything is limited to
what the user may view.
"""

import re

from django.contrib.contenttypes.models import ContentType
from django.contrib.contenttypes.prefetch import GenericPrefetch
from django.db.models import Count

from circuits.models import CircuitTermination
from dcim.models import Cable, CableTermination, Device, FrontPort, Interface, Location, PortMapping, RearPort
from dcim.utils import decompile_path_node

__all__ = ("site_data",)


def _named(obj):
    """{id, name} of a device (or anything named), its display text if unnamed."""
    if obj is None:
        return None
    return {"id": obj.pk, "name": getattr(obj, "name", None) or str(obj)}


# Interface types: copper ports (RJ45), and those with a transceiver cage or
# fixed optics (drawn as SFP). Others (virtual, wireless, backplane...) are
# not ports on a switch's front.
_ETHERNET = re.compile(r"^[\d.]+[mgt]?base-")
_COPPER = re.compile(r"^[\d.]+[mgt]?base-tx?$")
_CAGES = ("sfp", "xfp", "gbic", "cfp", "cxp", "cpak", "x2", "xenpak", "osfp", "cdfp")


def _port_class(interface_type):
    """"rj45", "sfp" or None for an interface of NetBox type `interface_type`."""
    t = (interface_type or "").lower()
    if _COPPER.match(t):
        return "rj45"
    if any(c in t for c in _CAGES):
        return "sfp"
    if _ETHERNET.match(t) and "base-k" not in t and not t.endswith("-t1"):
        return "sfp"
    return None


def _switch_ports(user, site):
    """Each device's physical ports, for switches: {device id: {count, type,
    uplinks}}, copper ports first and the others (SFP) as uplinks (all SFP:
    no uplinks); and each interface's place among them, in NetBox's order of
    their names: {interface id: socket}."""
    by_device = {}
    rows = (
        Interface.objects.restrict(user, "view")
        .filter(device__site=site, mgmt_only=False)
        .values_list("id", "device_id", "type")
    )
    for pk, device_id, interface_type in rows:  # in NetBox's interface order
        cls = _port_class(interface_type)
        if cls:
            by_device.setdefault(device_id, {"rj45": [], "sfp": []})[cls].append(pk)
    ports, positions = {}, {}
    for device_id, cls in by_device.items():
        main, uplinks = (cls["rj45"], cls["sfp"]) if cls["rj45"] else (cls["sfp"], [])
        ports[device_id] = {"count": len(main), "type": "rj45" if cls["rj45"] else "sfp", "uplinks": len(uplinks)}
        for n, pk in enumerate(main + uplinks, start=1):
            positions[pk] = n
    return ports, positions


def _cable_paths(user, site, visible):
    """The cabling from the site's interfaces on through patch panels:
    NetBox's cable paths, as the ids of their cables one after another. Only
    stretches of cables the user may view (`visible`), of two or more."""
    cable_type = ContentType.objects.get_for_model(Cable).pk
    rows = (
        Interface.objects.restrict(user, "view")
        .filter(device__site=site, _path__isnull=False)
        .values_list("_path__path", flat=True)
    )
    out = []
    for path in rows:
        run = []
        for step in path or []:
            for node in step:
                ct, pk = decompile_path_node(node)
                if ct != cable_type:
                    continue
                if pk in visible:
                    run.append(pk)
                else:
                    # Not to be joined across a cable they may not see.
                    if len(run) >= 2:
                        out.append(run)
                    run = []
        if len(run) >= 2:
            out.append(run)
    return out


def _choice(value, choices):
    if not value:
        return None
    return {"value": value, "label": dict(choices).get(value, value)}


def _termination(term, positions):
    """One end of a cable: {object_type, object_id, object}. A front port
    or interface also gets its `position` (`positions`: {model: {id:
    position}}): the socket it is on a patch panel or a switch."""
    obj = term.termination
    object_type = f"{term.termination_type.app_label}.{term.termination_type.model}"
    data = {"object_type": object_type, "object_id": term.termination_id}
    if obj is None:
        return data
    if isinstance(obj, CircuitTermination):
        circuit = obj.circuit
        data["object"] = {
            "id": obj.pk,
            "term_side": obj.term_side,
            "circuit": {
                "id": circuit.pk,
                "cid": circuit.cid,
                "display": str(circuit),
                "provider": _named(circuit.provider),
            },
        }
    elif hasattr(obj, "device"):
        data["object"] = {"id": obj.pk, "name": obj.name, "device": _named(obj.device)}
        position = positions.get(type(obj), {}).get(obj.pk)
        if position:
            data["object"]["position"] = position
    else:
        data["object"] = {"id": obj.pk, "display": str(obj)}
    return data


def site_data(site, user):
    """What the editor needs to draw `site`, as `user` may see it."""
    devices = Device.objects.restrict(user, "view").filter(site=site).select_related("role")
    locations = Location.objects.restrict(user, "view").filter(site=site)
    # Front ports per device, for patch panels: how many, and the type most
    # of them have.
    fronts = {}
    by_type = (
        FrontPort.objects.restrict(user, "view")
        .filter(device__site=site)
        .values("device_id", "type")
        .annotate(n=Count("id"))
    )
    for row in by_type:
        f = fronts.setdefault(row["device_id"], {"count": 0, "type": None, "most": 0})
        f["count"] += row["n"]
        if row["n"] > f["most"]:
            f["type"], f["most"] = row["type"], row["n"]
    # Each front port's place on its device, in NetBox's (natural) order of
    # their names: the socket it is on a patch panel.
    positions = {}
    count = {}
    in_order = (
        FrontPort.objects.restrict(user, "view")
        .filter(device__site=site)
        .order_by("device_id", "name")
        .values_list("id", "device_id")
    )
    for pk, device_id in in_order:
        count[device_id] = count.get(device_id, 0) + 1
        positions[pk] = count[device_id]
    switch_ports, interface_positions = _switch_ports(user, site)
    # A rear port behind one front port is at the back of that one's socket
    # (one behind several, an MPO trunk say, is not).
    fronts_of = {}
    for rear_id, front_id in PortMapping.objects.filter(device__site=site).values_list("rear_port_id", "front_port_id"):
        fronts_of.setdefault(rear_id, set()).add(front_id)
    rear_positions = {
        rear_id: positions[next(iter(fronts))]
        for rear_id, fronts in fronts_of.items()
        if len(fronts) == 1 and next(iter(fronts)) in positions
    }
    positions = {FrontPort: positions, Interface: interface_positions, RearPort: rear_positions}
    interfaces = (
        Interface.objects.restrict(user, "view")
        .filter(device__site=site, cable__isnull=False)
        .select_related("device")
        .annotate(ip_count=Count("ip_addresses", distinct=True))
    )
    cables = Cable.objects.restrict(user, "view").filter(terminations___site=site).distinct()
    terminations = (
        CableTermination.objects.filter(cable__in=cables)
        .select_related("termination_type")
        .prefetch_related(
            GenericPrefetch(
                "termination",
                [
                    Interface.objects.select_related("device"),
                    FrontPort.objects.select_related("device"),
                    RearPort.objects.select_related("device"),
                    CircuitTermination.objects.select_related("circuit__provider"),
                ],
            )
        )
    )
    paths = _cable_paths(user, site, set(cables.values_list("pk", flat=True)))
    ends = {}
    for term in terminations:
        side = "a_terminations" if term.cable_end == "A" else "b_terminations"
        ends.setdefault(term.cable_id, {"a_terminations": [], "b_terminations": []})[side].append(_termination(term, positions))

    mode_choices = Interface._meta.get_field("mode").choices or ()
    return {
        "site": {"id": site.pk, "name": site.name, "slug": site.slug},
        "devices": [
            {
                "id": d.pk,
                "name": d.name,
                "display": str(d),
                "role": {"id": d.role.pk, "slug": d.role.slug, "name": d.role.name} if d.role_id else None,
                "location": {"id": d.location_id} if d.location_id else None,
                "front_ports": (
                    {"count": fronts[d.pk]["count"], "type": fronts[d.pk]["type"]} if d.pk in fronts else None
                ),
                "switch_ports": switch_ports.get(d.pk),
            }
            for d in devices
        ],
        "locations": [
            {"id": loc.pk, "name": loc.name, "slug": loc.slug, "parent": {"id": loc.parent_id} if loc.parent_id else None}
            for loc in locations
        ],
        "interfaces": [
            {
                "id": i.pk,
                "name": i.name,
                "device": _named(i.device),
                "mode": _choice(i.mode, mode_choices),
                "count_ipaddresses": i.ip_count,
            }
            for i in interfaces
        ],
        "cables": [
            {"id": c.pk, "label": c.label, **ends.get(c.pk, {"a_terminations": [], "b_terminations": []})}
            for c in cables
        ],
        "paths": paths,
    }
