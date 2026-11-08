# This file is part of Stratagram: https://github.com/shinsterneck/stratagram
# Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
# SPDX-License-Identifier: Apache-2.0

"""NetBox Stratagram: a site's network diagram, drawn from its devices
and cables and edited in the browser on the site's Diagram tab."""

from netbox.plugins import PluginConfig

__version__ = "1.0.2"


class StratagramConfig(PluginConfig):
    name = "netbox_stratagram"
    verbose_name = "NetBox Stratagram"
    description = "Network diagrams of sites, edited in NetBox"
    author = "Shin Sterneck"
    version = __version__
    base_url = "stratagram"
    min_version = "4.5.0"
    max_version = "4.7.99"
    default_settings = {
        # Device type by device role (slug or name), before the built-in
        # guesses, e.g. {"patch-panel": "switch", "dmz-host": "server"}.
        "kinds": {},
        # Also draw devices without a cable to another device.
        "all_devices": False,
        # Each type of device on a layer of its own (Switches, Patch
        # panels...) and the cables on one, to show or hide them by type.
        "type_layers": True,
        # How the cables' lines run: "orthogonal", "straight" or "around"
        # (orthogonal, finding a way around devices in between).
        "routing": "orthogonal",
        # Device types drawn with their ports, as a switch's front: any of
        # "switch", "router", "firewall", "server".
        "fronts": ["switch"],
    }

    def ready(self):
        super().ready()
        # The Diagram tab on sites registers itself (before NetBox builds
        # its URLs on the first request).
        from . import views  # noqa: F401


config = StratagramConfig
