# NetBox Stratagram

Network diagrams of NetBox sites, drawn from their devices and cables and
edited in the browser on each site's **Diagram** tab, with the Stratagram
editor (the same as the desktop app, compiled to WebAssembly).

- The first time, the tab draws the site as NetBox has it: devices with
  their cabled interfaces as ports, cables as lines, laid out top down
  (internet and firewalls above, servers below). Device types come from the
  device roles, line types from the interfaces (802.1Q trunk, layer 3 when
  routed on both ends, layer 2 for access ports). Provider circuits become
  clouds. Patch panels (by role, or devices with front ports and no other
  role) are drawn with numbered sockets, one per front port, of the front
  ports' type (RJ45, LC or SC), and as wide as they need. Cables to front
  ports plug into their sockets, cables to rear ports come out behind the
  socket of the front port they map to. Switches show their front too: their
  physical interfaces as ports, odd ones on top and even ones below, copper
  ones first and the others as SFP uplinks, each cable in its port.
- Each of the site's locations gets a sheet of its own after the site's:
  the devices in the location (and in the locations inside it), plus the
  devices elsewhere they are cabled to, on the layer "Other locations"
  (marked with its name). A location's own **Diagram** tab shows the
  site's diagram open on that sheet.
- Each type of device is on a layer of its own (Internet, Firewalls,
  Routers, Switches, Patch panels, Servers...) and the cables on the layer
  Cables, to show or hide them by type; each device has a sub-layer of its
  own under its type's, named after it (renamed with it), to show or hide
  devices one by one (unfold the type's layer with ▸). The cabling stays end to end: with
  the patch panels hidden, a server's line goes straight to the switch port
  it is patched through to (NetBox's cable path), and hidden cables take
  their end-to-end lines with them. A diagram saved before this gets the
  layers at its next **Refresh from NetBox** (what you drew yourself stays
  where it is; a device you later move to another layer stays there).
- Each location is a layer of its own too, with the locations inside it
  and its devices' layers by type under it, and a background in a color of
  its own with a dotted border: on the drawing, every location is a tinted
  area holding its devices, the ones inside it nested in it. Auto layout
  keeps each location's devices together. A device moved to another
  location in NetBox moves to that location's layer at the next refresh.
- Arrange it, add notes, a frame, layers..., then **Save**: the diagram is
  kept in NetBox (one per site, in the change log).
- **Refresh from NetBox** opens a dialog with what NetBox has, as a tree
  like the layer list: locations, device types, devices and their
  interfaces. Nothing is selected at first; checking an entry selects
  everything under it, and unfolding a device lets you pick single
  interfaces (cables come along where both ends are chosen). Then choose
  **Update this sheet** or **Add as a new sheet**; a new sheet is laid out
  and shown at once. Updating adds the chosen devices and cables and
  renames what was renamed, leaving the rest alone. With everything (the
  site) selected, it is the full refresh: what is gone is set aside on the
  layer "Gone from NetBox", on every sheet, renamed locations rename their
  sheets and new locations get new sheets. Nothing you placed moves, and
  Ctrl+Z undoes it.
- Export SVG, PNG or draw.io from the editor's Export menu.
- Light, Dark and Neon themes (the selector in the editor's toolbar): the
  editor follows NetBox's own light/dark switch until you pick one, which
  is then remembered in your browser. SVG and PNG exports come out in the
  theme; the saved diagram keeps its colors.

Requires NetBox 4.5, 4.6 or 4.7.

## Install

Install it from [PyPI](https://pypi.org/project/netbox-stratagram/) into
NetBox's virtual environment:

```sh
sudo /opt/netbox/venv/bin/pip install netbox-stratagram
```

With [netbox-docker](https://github.com/netbox-community/netbox-docker), add
`netbox-stratagram` to `plugin_requirements.txt` and build the image (see
netbox-docker's guide to plugins; `collectstatic` is part of it).

To build the package yourself (needs Rust with the `wasm32-unknown-unknown`
target and `wasm-bindgen-cli` 0.2.129, see
[Building from source](https://github.com/shinsterneck/stratagram/wiki/Building-from-source), `pip install
build`, and `cargo install cargo-about` for the licenses of the libraries
built into the editor, which go into the package as
`THIRD-PARTY-LICENSES.txt`):

```sh
netbox-plugin/build.sh          # makes netbox-plugin/dist/netbox_stratagram-*.whl
```

and install that wheel instead. Then enable the plugin in `configuration.py`:

```python
PLUGINS = ["netbox_stratagram"]
PLUGINS_CONFIG = {
    "netbox_stratagram": {
        # Device type by device role (slug or name), before the built-in guesses.
        "kinds": {"patch-panel": "switch", "dmz-host": "server"},
        # Also draw devices without a cable to another device.
        "all_devices": False,
        # Each type of device on a layer of its own, the cables on one.
        "type_layers": True,
        # Each location on a layer of its own (its locations and its
        # devices' layers under it), with a background in its own color.
        "location_layers": True,
        # How the cables run, and new lines drawn by hand: "orthogonal",
        # "straight" or "around" (orthogonal, around devices in between).
        "routing": "orthogonal",
        # Device types drawn with their ports as a front, like a switch's:
        # any of "switch", "router", "firewall", "server".
        "fronts": ["switch"],
    },
}
```

Then, as for any plugin:

```sh
python3 manage.py migrate
python3 manage.py collectstatic --no-input
```

Your web server should serve `.wasm` files as `application/wasm` (nginx
does by default; elsewhere the editor still loads, a little slower).

## Permissions

The Diagram tab (of sites and locations) shows to those who may view the
site and *site diagrams* (`netbox_stratagram.view_sitediagram`); grant it
to everyone who should see the diagrams (superusers have it). A saved
diagram outside the constraints of their view permission is not shown
(nor can it be saved over). Drawn afresh or refreshed, the drawing only has
the devices and cables they may view.

Saving needs the permission to add (the first time) or change site
diagrams (`netbox_stratagram.add_sitediagram`,
`netbox_stratagram.change_sitediagram`), with their constraints: a diagram
saved outside them (constraints on `site__region`, say) is undone and the
save refused, as in NetBox's own edit views.

## License

Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version
2.0; the package includes the license and the NOTICE file.
