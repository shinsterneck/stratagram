# Stratagram

[![Build](https://github.com/shinsterneck/stratagram/actions/workflows/build.yml/badge.svg)](https://github.com/shinsterneck/stratagram/actions/workflows/build.yml)

Homepage: **<https://stratagram.app>**

A small, fast network diagram editor (formerly *netdiag*). Rust holds the model and all editing
logic; QML (Qt Quick, via [CXX-Qt](https://github.com/KDAB/cxx-qt)) renders
it on a `Canvas` and provides the toolbar and properties panel.

- Devices with a name, a type (router, switch, firewall, ...) and a fill color, drawn in a shape that shows the type
- Ports on any side of a device, with optional names (`Gi1/0/1`, `wan1`, ...)
- Links between ports: color, width, solid/dashed/dotted, straight, orthogonal or around-objects routing (finds a way around boxes in between),
  arrowheads at either end (arrow, open, diamond, circle) and a label
- Free text annotations (notes): multi-line, with font size, color and bold
- Save/load as readable JSON, export to SVG and to draw.io (editable)
- Undo/redo, snap to grid, grid on or off (**Grid** in the toolbar, Ctrl+'; remembered), multi-select, duplicate, pan and zoom
- Themes: **Light**, **Dark** and **Neon** (the selector in the toolbar; remembered), for the canvas, the
  panels and the SVG and PNG exports; see [Themes](#themes)

## Build

Needs Rust 1.85+ (edition 2024) and Qt 6.8 or newer with Qt Quick, Quick Controls, Quick Dialogs, Qt Svg and the QtCore QML module.

```sh
# Debian / Ubuntu
sudo apt install qt6-base-dev qt6-declarative-dev qt6-svg-dev qml6-module-qtquick \
  qml6-module-qtquick-controls qml6-module-qtquick-layouts qml6-module-qtquick-dialogs \
  qml6-module-qtquick-templates qml6-module-qtquick-window qml6-module-qtqml-workerscript \
  qml6-module-qt-labs-folderlistmodel qml6-module-qtqml-models qml6-module-qtcore
# Fedora
sudo dnf install qt6-qtbase-devel qt6-qtdeclarative-devel qt6-qtsvg-devel
# Arch
sudo pacman -S qt6-base qt6-declarative qt6-svg

cargo build --release
./target/release/stratagram [file.stratagram]
```

CXX-Qt finds Qt through `qmake`. If it picks up Qt 5 or nothing, point it at Qt 6:
`export QMAKE=/usr/lib/qt6/bin/qmake` (path varies by distribution, e.g. `qmake6` on Fedora).

Passing a file name opens it; if it does not exist yet, Ctrl+S saves to that name.

The version shows in the lower right corner of the window; `stratagram --version` prints it.
Releases are tagged `vX.Y.Z` in git.

### Windows

The [releases](https://github.com/shinsterneck/stratagram/releases) have an installer
(`stratagram-X.Y.Z-windows-x64-setup.exe`) and the same as a zip to unpack anywhere. The
installer asks whether to install for you or for everyone, adds Stratagram to the Start menu
and opens `.stratagram` files with it (a choice in the installer); the uninstaller takes the app,
its menu entry and the file type out again (your settings stay). `/VERYSILENT` installs without questions, `/ALLUSERS` or `/CURRENTUSER` picks
for whom.

On Windows, `stratagram.exe` is a window application: the command line modes (`--export-svg`,
`--layout`, ...) print to the console they are started from, but the prompt does not wait for
them; in scripts, wait with `start /wait stratagram ...` (cmd) or `Start-Process -Wait`
(PowerShell).

To build it yourself, install Rust (with the MSVC toolchain, through [rustup](https://rustup.rs) and the Visual Studio
Build Tools) and Qt 6.8 for MSVC 2022 64-bit with the Qt online installer, put Qt's `bin` folder
on the `PATH` (so `qmake` is found), then:

```powershell
cargo build --release
mkdir dist; copy target\release\stratagram.exe dist\
windeployqt --release --qmldir qml dist\stratagram.exe   # Qt's DLLs and QML modules next to it
```

The [Build workflow](https://github.com/shinsterneck/stratagram/actions/workflows/build.yml) does
the same on every push to `main`: each run has a `stratagram-windows` download with the
installer and the zip (the app, Qt and their licenses, `packaging/qt/`), next to the Linux
build, the NetBox plugin's package and a smoke test of the plugin in NetBox 4.5, 4.6 and 4.7.
The installer is made with [Inno Setup](https://jrsoftware.org/isinfo.php) 6 from
`packaging/windows/stratagram.iss`, then installed, tried and uninstalled in the same run:

```powershell
iscc /DVersion=1.0.0 packaging\windows\stratagram.iss   # dist\stratagram-1.0.0-windows-x64-setup.exe
```

The icon and version details are built into `stratagram.exe` (`build.rs`, from
`packaging/windows/stratagram.ico`).

### Releases

Pushing a tag `vX.Y.Z` that matches the version in `Cargo.toml` runs the
[Release workflow](https://github.com/shinsterneck/stratagram/actions/workflows/release.yml): the
whole build with its tests, then a GitHub release with the Windows installer, the Windows zip
and the NetBox plugin's package, with the tag's message as the release notes, and the plugin
on [PyPI](https://pypi.org/project/netbox-stratagram/) (trusted publishing from the `pypi`
environment, no token).

### Linux packages

- **Arch Linux and derivatives** (AUR): `yay -S stratagram-bin` installs the prebuilt app in
  seconds; `yay -S stratagram` builds it from source (needs Rust). Both install the menu entry,
  the icon, the `.stratagram` file type and the software center details below, and replace
  each other. Built from `packaging/aur-bin/PKGBUILD` and `packaging/aur/PKGBUILD`.
- **Other distributions:** each [release](https://github.com/shinsterneck/stratagram/releases)
  has `stratagram-X.Y.Z-linux-x86_64.tar.gz`, laid out like `/usr`. It is built on Arch Linux,
  so it needs Qt 6 as new as Arch's (currently 6.12) with Qt Quick and Qt Svg; on older
  distributions, build it from source. For your user only:

  ```sh
  tar -xzf stratagram-*-linux-x86_64.tar.gz --strip-components=1 -C ~/.local
  update-mime-database ~/.local/share/mime
  ```

### Desktop launcher (Linux, built yourself)

`packaging/` has what a desktop needs, named after the app's ID `app.stratagram.Stratagram`:
the `.desktop` entry (menu and launchers), the icon, the `.stratagram` file type and the
AppStream metainfo (what GNOME Software and KDE Discover show). The entry runs `stratagram`
from `PATH`, so install the binary first:

```sh
sudo install -Dm755 target/release/stratagram /usr/local/bin/stratagram
# for your user only:
id=app.stratagram.Stratagram
install -Dm644 packaging/$id.desktop ~/.local/share/applications/$id.desktop
install -Dm644 packaging/stratagram.svg ~/.local/share/icons/hicolor/scalable/apps/$id.svg
install -Dm644 packaging/$id.mime.xml ~/.local/share/mime/packages/$id.xml
update-mime-database ~/.local/share/mime
update-desktop-database ~/.local/share/applications
# or system wide: the same under /usr/local/share (with sudo), plus
# packaging/$id.metainfo.xml into share/metainfo/
```

The file manager then shows `.stratagram` files as Stratagram diagrams, with its icon, and
opens them with Stratagram on a double-click. (An older install used the name `stratagram`
for these files; remove `stratagram.desktop`, `stratagram.svg` and `stratagram.xml` from the
same places.)

### In the browser

The same editor runs in the browser: the core compiled to WebAssembly, with
the canvas and panel in plain JavaScript (`web/`). It is the base for editing
inside NetBox; on its own it opens and saves files on your computer.

Try it at **<https://stratagram.app/editor/>** (built from `main` with the homepage), or with an
example: <https://stratagram.app/editor/#file=../examples/berlin-hq.stratagram>. Everything runs in
the browser; diagrams are not uploaded anywhere. `#file=` opens a diagram from the same site
only.

It can be installed as an app (a PWA: `manifest.webmanifest`, `sw.js`): **Install app** at the
bottom right of the editor, shown when Chrome or Edge offer to install it (it opens their install
dialog); in Safari it tells how (*Add to Dock*, *Add to Home Screen*). It is hidden in the
installed app and where installing is not possible (Firefox). It
then runs in its own window, works offline (the service worker keeps the editor's files; online,
it always takes the newest from the site first) and, in Chrome and Edge, opens `.stratagram`
files from the system, saving back to them. Firefox has no install for web apps on the desktop;
there it works online like any page.

To build and serve it yourself:

```sh
rustup target add wasm32-unknown-unknown
cargo binstall wasm-bindgen-cli@0.2.129   # same version as in web/Cargo.toml
web/build.sh                              # builds into web/www/pkg
python3 -m http.server -d web/www 8000    # then open http://localhost:8000
```

Some desktop shortcuts belong to the browser there (Ctrl+N, Ctrl+Shift+N,
Ctrl+Shift+T, Ctrl+Shift+P); use the toolbar buttons instead. Save writes back
to the chosen file where the browser allows it (Chrome, Edge), and downloads
the file elsewhere. Exports are downloads.

To embed it in another page, `mount(element, { host })` from `web/www/app.js`
takes a `host` that says where diagrams come from and go to (see the comment
at the top of `app.js`).

### In NetBox

`netbox-plugin/` is a NetBox 4.5 to 4.7 plugin with this editor on every site's
**Diagram** tab: the site is drawn from its devices and cables (with a
sheet for each location, also shown on the location's own Diagram tab), saved in
NetBox, and refreshed from it later without moving what you arranged. See
`netbox-plugin/README.md`.

## Using it

| Action | How |
|---|---|
| Add device | Double-click empty canvas, or **+ Device** (Ctrl+Shift+N); the **▾** beside it adds one of a type ("Router 1", "Firewall 1"...) |
| Edit text in place | Double-click a device (name), port (interface name) or link (label) and type right on the canvas. Or select it and press F2 or Enter, or simply start typing to replace the text. The edit box grows with the text up to the visible canvas; longer text scrolls (scroll bars appear when it does not fit), and the corner handle resizes the box. Enter or Tab commits, Esc cancels, clicking elsewhere commits. Each edit is one undo step. |
| Connect | Drag from a device **border** or an existing **port** and drop on another device, its border or a port. Missing ports are created where you start and end. Dropping on the body picks the side facing the source, aligned for a straight line. |
| Auto layout | **Layout ▾** in the toolbar (Ctrl+L). Pick an arrangement and it applies at once; the menu stays open to try another, and Ctrl+Z undoes each one. **Hierarchy ↓ / →**: layers from the most upstream device (internet or cloud, then firewall, router, load balancer, switch, access point, servers and clients) downward or to the right, ordered to avoid crossing lines. Devices of the same type that share a neighbor, like the two firewalls of an HA pair on one switch, are treated as peers: they share a row and stay side by side, and what they both connect to is centered between them. Tiers of one type stay apart: two access switches on the same core switches are peers, and so is a core pair linked to each other with the same other neighbors, but a core switch and an access switch below it are not. Diagram layers set the rows too: devices on different layers never share a row, later layers (lower in the layer list) go below earlier ones, and the top of the hierarchy comes from the first layer. **Radial**: rings around that device, each branch in its own wedge. **Circle**, **Grid**: neighbors placed next to each other. **Force-directed**: an organic layout where linked devices pull together; it starts from the current positions, so running it again refines it. **Tidy**: cleans up the arrangement you have without redoing it: devices at nearly one height share a row, devices nearly above each other a column, a device linked to several in the row below (or above) is centered over them when that is a short move, overlaps are pushed apart (keeping the order, with room for line labels), the ports on each side are spread evenly (one in the middle, two at the thirds, and so on, in their order), so lines between devices that line up run straight; where two facing devices still do not line up, a lone port moves off the middle to keep its line straight. Devices move only a little, and the arrangement stays where it is. Works on the selected devices when two or more are selected, otherwise on all shown, unlocked devices; separate groups are laid out side by side, and the result stays where the devices were. Free connection points move with their device. **Spacing** sets the gap; lines with a label (or a type tag) get more where they need it: each line's **Label margin** (in the link panel, 24 by default) is kept free on both sides of its label, so the gap between rows (or columns) grows for the labels crossing it (stacked where they would sit side by side), devices in one row move apart for a labeled line between them, force-directed lengthens such lines, and radial, circle and grid keep that much room overall. **Move ports to face their links** puts each linked port on the side toward the other end (spread out so lines do not cross). **Route lines around objects** (on by default) makes sure no line runs through a device or text afterwards: a line that would gets the first routing that keeps it clear (its own, straight, orthogonal, else *around objects*); lines that are already clear keep their style. All three settings are remembered. |
| Deep zoom (objects inside objects) | Turn on **Zoom into** in the toolbar (Ctrl+I) and the view can zoom in without limit. Zoom into a device until it fills the view and add things inside it as usual (double-click, quick add, + Text, lines): they are made at that zoom level's scale, so they look normal now but are small inside the device, which itself keeps its size. Zoom back out and the diagram looks exactly as before, with the inner items fading out; zoom back in and they reappear. Items one level in only show beyond 400% (the most you can zoom without deep zoom), fully from 600%; without deep zoom they stay hidden. It works at any depth (each level is 8x smaller than the one around it), and the grid and snapping follow the level. While zoomed into a device, its inside is the background you work on (double-click adds, dragging selects) and its name shows as a small header; its border and ports still take lines. Things made inside a device while zoomed into it belong to it and stay inside it: dragging, nudging or resizing them (or a note's box) stops at its border. A device holds whatever lies completely inside it and in front of it: moving it moves its contents, auto layout keeps them with it, and lines from inside may leave it. Copies pasted while zoomed in are made at that level's size. The panel shows sizes and line widths as they look at the item's own level. Exports keep everything at its scale. |
| Device shapes | The **Type** list in the panel is grouped by category (Routing & switching, Security, Wireless, Servers & clients, Virtualization, Data center, Cabling, Internet & cloud); any other type can still be typed in. The type sets the outline and a symbol: router a pill with the classic four routing arrows (**ospf router**: the arrows in a ring; **bgp reflector**: a route bouncing off a mirror), switch a box with two rows of ports along the bottom, firewall a brick wall, load balancer a hexagon with one arrow fanning out into three, server a box with rack ears and a stack of rack units, cloud a cloud, internet a globe, client a monitor on a stand, access point a rounded box with radio waves (**standalone ap**: the waves over a dot in a ring, an AP on its own; **managed ap**: the waves on a dot tied down to a small controller square, an AP run by a controller), **wlan controller** a box with a controller unit and radio waves rising from it; no type (or a custom one) is a rounded box. **patch panel** (Cabling; quick add pp, patch) is a rack panel's front with mounting ears and numbered sockets: set **Ports** (1 to 288) and **Connector** in the panel: **RJ45** (the jack with its latch slot), **LC** (a duplex adapter with two ferrules) or **SC** (a square adapter). Sockets are grouped in sixes, up to 24 in a row; more go into further rows, filled evenly (48 ports: two rows of 24). The panel sizes itself to them: wider with more ports per row, taller with more rows; made narrower by hand, the sockets shrink to fit. Each socket takes a line like a port: drag from a socket to draw one, drop a line end or a connection point on a socket to plug it in, drag a socket's port to another socket to repatch it (onto the border, it becomes an ordinary port). The back of a socket (a rear port) is cabled at the panel's top or bottom edge right above or under it: draw a line to (or put a port on) the edge there, and it comes out exactly at its socket, with a dotted tie up to it; that socket's front cable then leaves at the other edge, so the way through the panel can be followed. The line plugs into the middle of the socket (of an LC duplex adapter, between its halves): the panel shows the plug and a cord to its edge, where the line goes on straight up or down. From NetBox, a device whose role says patch panel, or one with front ports and a role that says nothing else, becomes one with as many sockets as front ports, of the type most of them have (8P8C as RJ45, LC and SC variants; ST and FC are drawn as SC); cables to front ports plug into their sockets (by the front port's place among the panel's, in NetBox's order of their names), and cables to rear ports come out at the back of the socket of the front port they map to (a rear port behind several front ports, an MPO trunk say, stays on the border); refreshing updates the count and repatched sockets. A **switch** can show its front the same way: set **Ports** in the panel (0, the default, keeps it a box) with **Connector** RJ45 or **SFP** (a transceiver cage), and **Uplinks**, SFP ports in a group of their own at the right, numbered on from the last port (48 ports and 4 uplinks: 49 to 52). Odd ports are on top, numbered above, even ones below, numbered below (their RJ45 jacks upside down, as on a real switch), in groups of twelve; the switch widens to fit them. Its ports take lines like a patch panel's sockets, the line leaving at the edge that faces where it goes. Ports plugged into a socket (on a patch panel or a switch) are known by their socket's number: their names show only when you point at or select them. From NetBox, a switch gets its physical interfaces as ports: the copper ones (RJ45) first, the others (SFP and other transceivers, fixed optics) as uplinks, or all SFP when it has no copper ones, each cable in its interface's socket (by NetBox's order of the interface names). A switch already drawn as a box stays one when refreshed. **Routers** (also ospf router and bgp reflector), **firewalls** and **servers** can show their ports the same way: **Ports**, **Connector** (RJ45 or SFP) and **Uplinks** in the panel, 0 ports keeping them a box; a switch with ports turned into a firewall keeps them, while a patch panel's LC or SC sockets turned into a router leave it a box (they come back when it is a patch panel again). From NetBox they are drawn with their interfaces when the plugin's `fronts` setting names them (switches only by default). **virtual switch** (Virtualization; quick add vswitch, vss) is a rounded box with a switch's two arrows in a dashed frame; **distributed virtual switch** (quick add dvs, vds, dvswitch) has the arrows over three hosts tied together; draw.io export draws both dashed, CML export makes them unmanaged switches. **cloud proxy** (a cloud security proxy / SaaS secure web gateway; quick add proxy, swg) is a cloud with a shield. Cloud providers (**aws**, **azure**, **google cloud**, **alibaba cloud**, **telekom cloud**, **stackit**, and **zscaler** for the Zscaler proxy service, quick add zia; quick add also takes amazon, gcp, google, alibaba, aliyun, otc, telekom) are clouds with a tile in the provider's color and a monogram (simple stand-ins, not the providers' logos); with devices inside, they become a group box with the tile in its top left corner, like draw.io's AWS Cloud groups. The tile is exported to SVG and draw.io too. Symbols sit left of the name, or small above it when the name is too long to share the row (and are left out when there is no room for either). The shape stays within the device's box, where its ports sit. Devices with others inside keep their outline but leave out the symbol and inner lines. SVG export draws the same; draw.io export uses its nearest built-in shape (pill, hexagon, process, cloud, ellipse) without the symbols. |
| Quick add | Type a comma-separated list into the toolbar field (Ctrl+K) and press Enter. `Firewall, Router, Switch` adds a firewall, a router and a switch, each labeled with what you typed; `firewall\|my-box, rt\|test1` adds a firewall labeled my-box and a router labeled test1: the type first, then `\|` and the name. Kinds are matched ignoring case and spacing (short forms fw, rt, rtr, sw, lb, srv, ap, wlc, sap for a standalone AP and lap for a managed one also work); an unknown kind before `\|` is kept as a custom kind, and a bare unknown word becomes a plain device with that label. Repeated bare entries are numbered (Router, Router 2). The devices are placed in rows of six in the middle of the view, moved down past anything already there, selected, and undone as one step. Semicolons and new lines also separate entries. |
| Insert into a line | Drag a device onto a line so the line runs through its middle: the line lights up green, and dropping the device there puts it in between. The line now ends at a new port on the device, and a second line with the same type, label and style goes on from another new port to where the first went (each port on the side facing its end; the start arrow stays on the first half, the end arrow moves to the second). One undo step. Deleting a device joins its lines back up: lines with the same type and label pair up, and a pair that is the only one with that type and label becomes one line between their far ends. Lines that do not pair up are removed with the device, as before. |
| Bend a line | Drag a line (not its ends) and a connection point appears where you grab it: the line is split there into two that both attach to the point, and the point follows the mouse. Each half keeps the line's style; the label and start arrow stay on the first half, the end arrow moves to the second. Bend again to add more corners, drag the point later to reroute, or drop it on a device border to route the line through a port there. Deleting a bend point (one with exactly two lines) joins them back into one line. A plain click still just selects the line. A line bent through points is drawn as one line: where exactly two lines of the same type meet at a point, there are no arrowheads or end marks (the L2 squares, L3 circles), a tunnel's band runs on through the bend, and the type's tag shows once (with the label); on screen, in SVG/PNG and in draw.io. Lines of different types meeting at a point keep their ends. |
| Layer background | The square icon in a layer's row opens its background: a rectangle behind the layer's devices and texts, grouping them, with a **margin** around them. Choose **rounded** (with a radius) or **square** corners, a **border** (none, solid, dashed, dotted) with its width and color, and a **fill** color with its **opacity** in percent. It follows the devices as they move and resize, sits under everything else (clicks go through it), is left out while the layer is hidden or empty, and the frame and Fit include it. Lines do not count, since they may run to other layers. SVG, PNG and draw.io show it (on the layer's draw.io layer, skipped when read back). The icon shows its fill and border; it is outlined when the layer has none. |
| Frame (border with header and footer) | With nothing selected, pick a **Frame** template in the panel; the list is grouped by category. *Basic*: **lines** (a plain outline, title in a ruled header, footer below), **banner** (a colored header band with the title in white and a tinted footer band), **minimal** (no outline: a large title above, a thin rule and small print below), **card** (a rounded card with a soft shadow). *Business*: **corporate** (an accent bar down the left, company name and logo top right, one line of small print), **report page** (centered title, rules, page number in the middle, the note in capitals on the right), **presentation** (a 16:9 slide around the diagram: large title, slim footer with logo, page number and an accent bar), **executive summary** (a title band and a summary text panel beside the diagram, wrapped to fit), **customer proposal** (the customer's logo left and yours right, "Prepared for" and "Prepared by", a confidentiality notice). *Technical drawing*: **title block** (double border and a labeled table along the bottom with title, author, note, page and last change), **ISO title block** (coordinate zones 1, 2, 3… and A, B, C… around the edges and a title block in the bottom right corner with drawing number, revision, sheet, drawn and checked by, and date), **revision block** (the title block with a revision table above it: one row per line in the panel, *rev | date | change | by*), **blueprint** (white lines and text on blueprint blue with a grid; the diagram keeps its colors), **as-built** (a red AS-BUILT stamp and a handover table: installed by, commissioned, accepted by with a signature line, last change, page). *Documentation & governance*: **controlled document** (a document control table with ID, version, classification, owner, next review and last change; "Uncontrolled when printed" in the footer), **sign-off** (three signature boxes: prepared, reviewed and approved by, each with signature and date lines), **solution design** (version, owner, reviewers and a status badge: draft, in review, final), **change request** (change ID under the title and a status badge: draft gray, approved green, implemented blue, rejected red; footer with author and approver), **classification banner** (bars at the top and bottom with the level: PUBLIC green, INTERNAL blue, CONFIDENTIAL amber, RESTRICTED red). *Operations*: **site sheet** (site name, address and a site code box in the header, and a legend in the footer of the line types the diagram uses, with a sample of each), **data center location** (a location path like EU › Berlin › HQ › Room 2.14 above the title and the location code in a box), **incident report** (a severity badge P1 critical to P4 low and a colored top bar, incident ID, start and end, affected services), **NOC display** (dark wall display with a status pill: operational, degraded, outage, maintenance; dark lines show little contrast on it). The panel shows the fields the chosen template uses. Any template can carry a **watermark** (e.g. DRAFT, CONFIDENTIAL, SAMPLE: large, faint and diagonal over the diagram; red for confidential or restricted) and a **second logo** (the customer's: left in the customer proposal, otherwise beside the first). Every template can show a page number (e.g. 1/3), the file name and a **logo** (a PNG or JPEG of up to 1 MB, kept inside the .stratagram file so it travels with it). Choose whether to show the date and time of the last change, the margin and the color. **Defaults** for the fields go into the configuration file `~/.config/stratagram.conf` (where the app keeps its other settings; `$XDG_CONFIG_HOME` is honored; on Windows `%APPDATA%\stratagram.conf`, on macOS `~/Library/Application Support/stratagram.conf`): keys under `[frame]` apply to every template, keys under `[frame-<template>]` (e.g. `[frame-site]`, `[frame-classification]`) override them for that one. A new frame takes all of them; switching templates fills in only what is still empty, so nothing typed is lost. `logo=` takes the path of a PNG or JPEG, `auto=true` gives every new diagram a frame right away (with `template=`, default lines). **Save as default** in the Frame section writes the current frame's reusable fields there for you: **All templates** into `[frame]` (with the template, which `auto=true` uses), **Only <template>** into that template's section. Saved are author, company, note, color, margin, the show date and file name options, classification level, owner, reviewer, approver, installer and both logos (copied to `~/.config/stratagram/logos/`, so they keep working when the diagram moves); title, subtitle, IDs, versions, dates, customer, status and revisions stay with the diagram. Fields left empty are taken out of that section; the rest of the file is kept. The file is read whenever a frame is made, so changes apply without a restart; values that do not fit (e.g. `margin=wide`) are reported in the status bar. The frame sizes itself to everything shown plus the margin, widens to fit its texts, and follows the diagram as it changes. It lives on its own **Frame** layer at the bottom of the stack, which holds only the frame: it cannot be the active layer, items cannot be moved onto it, and items found on it in a file move to the first other layer. Untick the layer to hide the frame, remove the layer (or pick *none*) to drop it; clicks go through it. The time of the last change is saved with the file. SVG export uses the frame as the page; draw.io gets it as plain shapes on the Frame layer (left out when opened or pasted back into stratagram, and not copied to the clipboard). |
| Frame defaults (example) | <pre>[frame]<br>auto=true<br>template=site<br>author=Shin<br>company=ACME Networks<br>color=#0f766e<br>logo=/home/shin/Pictures/acme.png<br>show_file=true<br><br>[frame-site]<br>doc_id=BER-01<br>address="Hauptstr. 1, Berlin"<br><br>[frame-classification]<br>level=confidential</pre>Keys: title, subtitle, author, note, company, customer, doc_id, version, revision, owner, review_date, reviewer, approver, status, level, severity, start, end, services, address, location, installer, commissioned, summary, page, watermark, show_date, show_file, margin, color, logo, logo2. Sections per template use the names in the file format: `[frame-proposal]`, `[frame-incident]`, `[frame-asbuilt]` and so on. |
| Line types | **Type** in the link panel says what a line carries: **trunk (802.1Q)** is a double line, **Layer 2 (switched)** a line with a small filled square near each end for the switch ports (tag L2), **Layer 3 (routed)** a line with a small open circle near each end for the IP interfaces (tag L3), **L3 subinterface (802.1Q)** the double line of a trunk with those circles (tag L3 subif; put the subinterfaces or VLANs in the label), **IPsec**, **VXLAN** and **CAPWAP** (access point to wireless LAN controller) tunnels are a tinted band around the line (its edge solid for IPsec, dashed for VXLAN, dotted for CAPWAP), **SD-WAN** lines are dashed by default (choosing SD-WAN turns a solid line dashed, and leaving it turns it solid again; other dash choices are kept), and **MPLS** lines get short strokes across them, like a label stack. Each shows its tag (802.1Q, IPsec, VXLAN, CAPWAP, SD-WAN, MPLS, L2, L3, L3 subif) in bold in the label box in the middle, before the label. A label that would overlap another one slides along its line to a free spot (keeping its label margin to the ends where it can), on the canvas and in every export. To place a label yourself, drag it: it moves only along its line (to the point of the line nearest the mouse, around bends too) and stops before the ends, so it never covers a port. On a line bent through points it moves on through them, along the whole line, onto the next piece (up to a piece with a label of its own); only the line's real ends stop it. Hold Ctrl while dragging it to line its middle up with devices (edges and centers), ports, points, texts and other line labels, with a guide. The spot is saved with the line and used in every export; other labels keep clear of it. **Auto position** in the link panel lets it be placed automatically again. A click on a label selects its line, a double-click edits it. Color, width, dashes and arrows still apply; bulk editing sets the type of many lines at once. SVG export draws the same; draw.io gets a double line (`shape=link`) for trunks and L3 subinterfaces and a band (`flexArrow`) for tunnels (MPLS strokes and the L2 and L3 end marks are left out), with the tag in the label, and reads the type back when pasted or opened again. **Encapsulation**: below Type, **Carried over** adds what the line is transported over, as many layers as needed (e.g. VXLAN over IPsec, or VXLAN over IPsec over MPLS); **Remove outer** takes the outermost away. Each carrier is a tinted band around what it carries, outermost widest, its edge solid or dashed by type (dashed VXLAN, dotted CAPWAP, long dashes SD-WAN), and the tag reads "VXLAN over IPsec". SVG/PNG draw the nested bands; draw.io gets one band as wide as the outermost and the full tag, and reads the layers back. |
| Lines to nowhere (connection points) | Drop a line on empty space (or on a text or another line) and it ends at a free x,y connection point, so it looks attached to nothing, e.g. an arrow pointing at something outside the drawing. The point shows as a small ring in the editor and is never exported; draw.io gets a loose edge end there. Drag the ring to move the end; drop it on a device border and it becomes a port there (or joins the port it is dropped on), with its lines attached (the device lights up while you hover). The reverse works too: pull a port that has lines off its device and it becomes a free point that keeps them. Ctrl+drag lines a point up with ports, Shift+drag from it draws another line, and lines dropped onto it attach to it. A point can carry a **label** (in the panel, or double-click the point): it sits just beyond the end of its line, continuing its direction, so an arrow ending at a point labeled "Internet" points at the word; the point stays invisible in exports, its label does not. **+ Point** (Ctrl+Shift+P) adds a point on its own, e.g. to draw a free-standing arrow between two points. X/Y can be typed in the panel. Points live on layers, can be selected, duplicated and deleted (deleting one removes its lines, unless it is a bend between exactly two). Very short drags onto empty space are ignored as slips. |
| Add text annotation | **+ Text** (Ctrl+Shift+T) puts a note in the middle of the view and starts editing it. Shift+Enter starts a new line, Enter commits; a note left empty is removed. A note fits its text unless you give it a box: resize the editor with its corner handle (the note keeps that size when you finish), or drag the resize handle of a selected note. In a box the text wraps to its width and stays inside it, so it stays inside a device you zoomed into: on the canvas a scroll bar appears when the text is taller than the box (scroll with the wheel over the note, Ctrl+wheel still zooms, or drag the bar); exports show the top of the text, cut at the box with "…". **Fit to text** in the panel undoes the box. Move, multi-select, nudge, duplicate and delete it like a device; set size, bold and color in the panel. |
| Add port | Double-click a device border (then type its name), or use the Top/Right/Bottom/Left buttons in the panel |
| Move port | Drag it along the device border, also around corners onto another side; it snaps to the grid when snap is on. Pulling an unused port away from the device draws a link instead (the port stays put); pulling a port that has lines off the device detaches it into a free connection point that keeps them. Shift+drag always moves. For an exact spot, set side and "Position along side" in the panel. |
| Resize device | Drag the small handle at the bottom-right corner of a selected device |
| Align (smart guides) | Hold **Ctrl while dragging** a device, note or selection: it snaps into line with nearby devices and notes (left/center/right edges, top/middle/bottom) and its ports snap level with ports on other devices, so links become straight. Ctrl+dragging a port slides it into line with a port on another device. Ctrl+dragging a line label slides it along its line to where it lines up with them. A red dashed guide shows what it lined up with. It also snaps to equal spacing, and pink distance markers show the equal distances: midway between two boxes in a row or column; repeating the gap of the neighboring pair when you drag an outer box of a row or column (also to center the box in the middle); and halfway between two boxes that sit diagonally from each other (measured center to center). Press or let go of Ctrl mid-drag to switch it on or off. A Ctrl+click without moving still toggles the selection. |
| Layers | The **Layers** list at the bottom of the properties panel. **+ Layer** adds one and makes it active; every new device, link, text, quick-add or duplicate goes into the active (highlighted) layer, which you choose by clicking it. Double-click to rename. The tick box shows or hides a layer: hidden items cannot be clicked or selected, a link is hidden when its own layer or either of its devices is, and adding something to a hidden active layer shows it again. The padlock **locks** a layer: its items stay visible but are read only. They cannot be clicked, selected, moved, edited or connected to (clicks go through to what is underneath, and lines cannot be dropped on locked devices), new items cannot be added while a locked layer is active, and a locked layer cannot be removed. Locked items still serve as Ctrl+drag alignment targets. The tag icon shows the layer's name in the lower right corner of each of its devices (on screen and in both exports); it is off by default and can be turned on per layer. **+ Sub-layer** adds a layer under the active one: a layer's sub-layers are listed indented under it, and its ▸ / ▾ shows them or folds them away. Hiding or locking a layer hides or locks the layers under it too (their own tick boxes keep their state, greyed out); auto layout keeps sub-layers with their layer; removing a layer removes the layers under it; draw.io export writes sub-layers as layers named after both ("Servers / srv-01"). Drag a layer up or down to change the order (an insertion line shows where it goes; the layer it would be under lights up lightly), or onto the middle of another layer to put it under that one (which lights up as its new parent); its sub-layers move with it, as deep as layers go. One undo step each; a layer cannot go under itself, and the frame's layer stays. Changing layers (showing, hiding, locking, choosing one) leaves the list where it was scrolled to. **Remove** deletes the active layer with everything on it (asks first if it is not empty; Ctrl+Z restores it); the last layer cannot be removed. Move a selection to another layer with the **Layer** box in the panel. **Ports** can be on a layer of their own (the **Layer** box of a selected port; by default a port is on its device's layer): hiding that layer hides the port and its name, while its lines stay, and locking it makes the port read only. New ports go into the active layer when that is not their device's, so with an "Interfaces" layer active, the ports you make (also by drawing lines) land on it. Removing a layer puts its ports back on their devices' layers. Layers are saved in the file; SVG export leaves hidden layers out, draw.io export writes them as draw.io layers (hidden ones hidden). Cabling that runs on through patch panels stays end to end, whether drawn by hand (a line into a socket at the front, one from the same socket's back) or from NetBox (its cable paths): hide the layer of the patch panels and the server's line goes straight to the switch port it reaches, looking like the line it starts with; hide only one of two panels and it runs from the other one on. These lines are drawn on screen, in SVG and PNG, routed afresh clear of what is shown. Clicking one selects the first cable it stands for (its type, color and label set the line) with the ports at both ends lit, and pointing at one tells what it connects end to end; they are not bent by dragging (show the layer to change the cabling), and draw.io export does not have them. |
| Sheets | A document can hold several sheets (pages), shown as tabs under the canvas. Click a tab to switch, **+** adds an empty sheet after the current one, **double-click** a tab to rename it in place (Enter keeps the name, Escape cancels), **drag** a tab sideways onto another to move it there (a blue bar shows where; the sheet in view stays in view), and the **×** on each tab removes it with everything on it, asking first unless it is empty; the last sheet cannot be removed. Adding, removing, renaming and moving sheets are undone with Ctrl+Z like any edit, and undo or redo shows the sheet the change was on. Each sheet keeps its own zoom and position. All sheets are saved in the one file; draw.io export writes one page per sheet, named after it, while SVG and PNG show the sheet in view (the first sheet from the command line). |
| Stacking order (z) | One front-to-back order shared by all layers, so an item on any layer can sit in front of or behind items on any other. New items go on top. **Arrange** in the panel, or (with the canvas focused) Home to front, Page Up or Ctrl+] one step forward, Page Down or Ctrl+[ one step backward, End to back. A step moves past the next item that overlaps it. Links take part too, so a link can be drawn over a device. Older files keep their look: links under devices, texts on top. |
| Multi-select | Drag a rectangle on empty space (selects the devices, texts and links fully inside it); Ctrl+click or Shift+click toggles devices, texts and links; Ctrl+A selects everything shown |
| Bulk editing | With several items selected, the panel edits them all at once, one section per kind: devices (type, fill color, width, height), texts (size, bold, color), links (type, label, label margin, arrowheads, line style, routing, width, color), plus layer and stacking for the whole selection. Values that differ show as "(mixed)" and are only changed when you set them. **Rename** numbers the selected devices in reading order (rows top to bottom, then left to right): `#` is replaced by the number, `##` pads it to two digits (`sw-##` gives sw-01, sw-02, …); without `#` the number is appended. Every change is one undo step. |
| Link style | Select a link; set arrows, line style, routing, width, color and label in the panel. New links use the style you set last. Untick **Show label** to hide a line's label and its type tag, on screen and in exports (the text and type are kept, draw.io gets the label hidden); this also works for several lines at once, and typing a label in place shows it again. |
| Delete / duplicate | Del or Backspace / Ctrl+D (duplicates keep links between the copied devices) |
| Nudge | Arrow keys (10 px), Shift+arrows (1 px) |
| Pan / zoom | Middle or right drag, or Space+drag / mouse wheel, Ctrl+= and Ctrl+-, Ctrl+0 to fit |
| Properties panel | Click the thin strip at its left edge, the **Panel** toolbar button or Ctrl+B to collapse it into the right edge and give the canvas the full width. The choice is remembered. |
| Undo / redo | Ctrl+Z / Ctrl+Shift+Z or Ctrl+Y |
| Files | Ctrl+N, Ctrl+O, Ctrl+S, Ctrl+Shift+S, Ctrl+E (export SVG), Ctrl+Alt+E (export PNG), Ctrl+Shift+E (export draw.io) |

## Themes

The selector in the toolbar switches between three looks. The app remembers the choice (the web
editor per browser), and it changes the canvas, the panels around it, and the SVG and PNG exports:

- **Light**: the diagram as it is drawn, on white.
- **Dark**: a dark navy background; each light fill turns deep in its own color (a light blue switch
  becomes deep blue) with a bright border of the same color, lines and text turn light.
- **Neon**: near-black fills, fully saturated outlines and icons, cyan lines, and a soft glow on lines
  and outlines (on the canvas and in SVG; PNG has no glow).

A theme never changes the file: diagrams keep the colors they were drawn in, and a dark theme
translates them when drawing. Colors you picked yourself follow the same rule (they keep their hue),
cloud provider tiles keep their brand colors, and a diagram made in one theme looks right in the
others. draw.io export and the clipboard copies keep the file's colors, since draw.io edits them
(and has a dark mode of its own). In NetBox the editor follows NetBox's light/dark switch until you
pick a theme in the editor.

## Native dialogs

On Linux, open, save and export use the desktop's own file dialogs through the XDG desktop portal (GNOME, KDE, and on Hyprland, Sway and other wlroots desktops the GTK portal), so they show your bookmarks and look like the rest of the desktop. If no portal answers, Qt's built-in dialog is used. To choose differently, set `QT_QPA_PLATFORMTHEME` yourself (e.g. `gtk3`); stratagram only picks the portal when it is unset. The interface itself stays light even when the desktop prefers dark.

## Export

**SVG** is a static picture of the diagram (shown layers only), for documents and wikis, in the colors of the [theme](#themes) you work in.

**PNG** is the same picture as pixels, for chat, tickets, slides and anything that does not take SVG: rendered from the SVG export by Qt, so frame, logos and watermark look the same, on a white (or, in a dark theme, dark) background, at 2 pixels per diagram unit (sharp on high resolution screens and in print). Very large diagrams are scaled down to at most 16384 pixels on their longer side.

**draw.io** (`.drawio`) is an editable diagram for draw.io / diagrams.net, including
the Confluence and VS Code integrations:

- devices become rounded boxes with their colors; the device type is kept as a
  custom property (Edit Data in draw.io)
- ports become small squares inside their device, so they move with it
- links connect port to port, so they stay attached when you move things in draw.io;
  arrowheads, dash style, width, color, orthogonal routing and labels carry over;
  lines routed around objects keep their bends as waypoints
- interface names become labels near the matching end of the link
- notes become borderless text boxes
- layers become draw.io layers (hidden ones stay hidden), and the stacking order carries over

**Copy for draw.io** (Ctrl+C on the canvas, or in the Export menu) puts the selection on the
clipboard as draw.io shapes; with nothing selected it copies the whole shown diagram. Paste it
into draw.io (or diagrams.net, Confluence, VS Code) with Ctrl+V and it arrives as editable
shapes in the current layer, with the same styles as the draw.io export. Lines between copied
devices come along; a selected line whose device is not selected keeps a loose end there.
Hidden layers are left out.

**Paste** (Ctrl+V on the canvas, or in the Export menu) works the other way too:
- stratagram's own copies paste back exactly (ports, names, styles, connection points), 20 px
  down and right of the original, a step further each time;
- shapes copied in draw.io become devices (the type is guessed from the shape, e.g. a Cisco
  firewall icon becomes a firewall), texts, links (with arrows, dashes and routing) and loose
  ends; groups and layers are flattened;
- any other text becomes a text note.

Pasted items go into the active layer, in front, selected, as one undo step; Ctrl+X cuts.

**Copy sheet with layers and frame** (in the *Export* menu) copies the whole sheet in view, everything
on it (hidden layers too), as text to paste into Stratagram: in the same editor, another window, the
app or NetBox's Diagram tab, either way. Pasted with Ctrl+V, its items keep their layers (those of the
same names, or new ones with the same settings: hidden, locked, name shown, background) and the
frame comes along when the sheet has none (a sheet with a frame of its own keeps it). Into an empty
sheet it comes as it was, where it was and still standing for the same NetBox objects; into a sheet
with things on it, it goes to their right, as copies. One undo step.

**Cisco Modeling Labs**: *Export CML lab…* (or `--export-cml`) writes the sheet in view as a CML lab
topology (YAML, schema 0.2.1, which CML 2.x imports), to build and run the drawn network as a lab.
Each device becomes a node by its type: routers (also OSPF router, BGP reflector and the AWS gateways)
IOSv, switches IOSvL2, firewalls ASAv, servers and load balancers Ubuntu, clients Desktop, the internet
and clouds an external connector (an unmanaged switch when several lines meet there), anything else an
unmanaged switch. Each line end gets an interface of its own, named as the platform names it
(GigabitEthernet0/0, ens2, port0...); the IOS style platforms start with the device's name as hostname
and the drawing's port names as interface descriptions. A line bent through points is one link between
its real ends; tunnels (VXLAN, IPsec, CAPWAP, SD-WAN) and lines to a free point are not cables and are
left out (the status line says how many). Text notes become text annotations; hidden layers are left
out. CML must have the node definitions it uses (the reference platforms do).

All exports also work without the GUI, for scripts and pipelines (the PNG scale, pixels per unit, is optional and 2 by default;
`--theme dark` or `--theme neon` exports SVG and PNG in that theme, light by default):

```sh
stratagram --export-drawio site.stratagram site.drawio   # every sheet as a page
stratagram --export-svg site.stratagram site.svg
stratagram --export-cml site.stratagram site-lab.yaml
stratagram --export-png site.stratagram site.png 3
stratagram --export-png site.stratagram site-dark.png --theme dark
```

Files written by scripts can leave the placement to the app: devices need no
`x`, `y`, `w`, `h` or `color`, ports no `side` or `t`, and device types may use
short forms (fw, sw, srv, tgw...). `--layout` then runs an auto layout on every
sheet and saves the result (OUT may be the same file):

```sh
stratagram --layout hierarchy generated.stratagram generated.stratagram [SPACING]
```

The layouts are those of the Layout menu: hierarchy, hierarchy-lr, radial,
circle, grid, force and tidy.

## File format

Diagrams are saved as `.stratagram` files: plain JSON, easy to generate from scripts (for example
from NetBox data). Files from before the rename (`.netdiag`, format `"netdiag"`) and older `.json` files
still open, and keep their name when saved with Ctrl+S; a file with several sheets holds the first one at the
top level, as below, and the others in `"sheets"` (see after the example); Save as suggests the `.stratagram` name. On
first start, the settings of netdiag (`~/.config/netdiag.conf` and its saved logos) move over to
`~/.config/stratagram.conf`; draw.io files exported by netdiag still read back with their line types.

```json
{
  "format": "stratagram",
  "version": 1,
  "devices": [
    { "id": 1, "name": "fw-01", "kind": "firewall", "x": 300, "y": 90, "w": 120, "h": 60,
      "color": "#fecaca", "layer": 20, "z": 1,
      "ports": [ { "id": 6, "name": "port3", "side": "bottom", "t": 0.5 } ] }
  ],
  "links": [
    { "id": 8, "from": { "device": 1, "port": 6 }, "to": { "device": 2, "port": 7 },
      "color": "#334155", "width": 2.0, "style": "solid",
      "head_start": "none", "head_end": "arrow", "routing": "straight", "kind": "trunk", "label": "10G", "label_margin": 32,
      "layer": 20, "z": 0 },
    { "id": 10, "from": { "device": 1, "port": 6 }, "to": { "point": 11 }, "head_end": "arrow" }
  ],
  "points": [ { "id": 11, "x": 360, "y": 300 } ],
  "notes": [
    { "id": 9, "text": "DMZ\nVLAN 20", "x": 280, "y": 20, "size": 14, "color": "#334155", "bold": false,
      "layer": 21, "z": 2 }
  ],
  "layers": [
    { "id": 20, "name": "Physical", "visible": true },
    { "id": 21, "name": "Annotations", "visible": false, "locked": true, "show_name": true }
  ],
  "active_layer": 20
}
```

`t` is the position along the side (0 = left/top, 1 = right/bottom). A note's `x`/`y` is the
top left corner of its text, and optional `w` and `h` give it a fixed box (at its own scale); `notes` may be left out. A link's `label_at` (0 to 1 along it) holds a label placed by hand, and `"label_hidden": true` hides its label. A port's `layer` puts it on a layer of its own (left out for its device's). A link's `kind` (plain, trunk, ipsec, vxlan, capwap, sdwan, mpls, l2, l3, l3sub) with an optional `over` (the kinds carrying it, innermost first, e.g. `["ipsec"]`) may be left out for a plain line. A layer may carry a `background` (`margin`, `fill`, `opacity` in percent, `border`: none, solid, dashed or dotted, `border_color`, `border_width`, `rounded`, `radius`). A layer may carry a `frame` (`template`: lines, titleblock, banner, minimal, corporate, report, card, iso, revision, blueprint, change, classification, site, controlled, signoff, solution, proposal, presentation, executive, asbuilt, incident, location or noc; `title`, `subtitle`, `author`, `note`, `show_date`, `margin`, `color`, and as needed `company`, `doc_id`, `revision`, `approver`, `status`, `level`, `address`, `page`, `show_file`, `version`, `owner`, `review_date`, `reviewer`, `customer`, `summary`, `installer`, `commissioned`, `severity`, `start`, `end`, `services`, `location`, `watermark`, `revisions` (`rev`, `date`, `change`, `author`), `logo` and `logo2` (data URIs, with `logo_w`/`logo_h` and `logo2_w`/`logo2_h`)), and `changed` holds the local date and time of the last change. A link end is either a port (`device` + `port`) or a free connection point (`point`, with an optional `label`); `points` may be left out. `z` is the global stacking order
(higher is in front, across all layers). `layers`, `layer` and `z` are optional: without them
everything goes into one layer, with links under devices and notes on top. Ids only need to be unique within the file; links pointing at missing ports are dropped on load.
A document with several sheets keeps its first sheet at the top level and the others in `sheets`, a list of the same
fields (`devices`, `links`, `notes`, `points`, `layers`, `active_layer`, `changed`) without `format` and `version`;
each sheet's `name` is optional (`"name": "Core"`). A one-sheet file has neither, so older versions open the first
sheet of any file.
See `examples/demo.stratagram` and its exports `examples/demo.svg` and `examples/demo.drawio`.

## Code layout

A Cargo workspace: `core/` is the library **stratagram-core** (everything
without a user interface; it also builds for WebAssembly), the desktop app
(Qt) is the package at the top.

| File | Purpose |
|---|---|
| `core/src/model.rs` | Devices, ports, links, notes, connection points, layers, stacking order, JSON format |
| `core/src/geom.rs` | Link routing, arrowheads, label placement, colors (shared by screen and SVG) |
| `core/src/editor.rs` | Hit testing, mouse/drag state machine, selection, undo/redo, property edits, scene JSON for QML |
| `core/src/layout.rs` | Auto layouts (hierarchy, radial, circle, grid, force-directed) |
| `core/src/config.rs` | Reads `stratagram.conf` (the frame defaults) |
| `core/src/frame.rs` | The frame around the diagram: sizing, the templates and the logo, as shapes shared by the canvas and both exports |
| `core/src/shape.rs` | Device outlines by type, as path data shared by the canvas and SVG export |
| `core/src/svg.rs` | SVG export |
| `core/src/drawio.rs` | draw.io export (also the clipboard copy) |
| `core/src/drawio_import.rs` | Reading draw.io shapes for pasting |
| `src/bridge.rs` | CXX-Qt `DiagramController` QObject exposed to QML |
| `cpp/clipboard.h` | Clipboard access (cxx-qt-lib has no QClipboard binding) |
| `qml/DiagramCanvas.qml` | Draws the scene, grid, pan/zoom, mouse and key input |
| `qml/PropertiesPanel.qml` | Context-sensitive editor for the selection |
| `qml/main.qml` | Window, toolbar, shortcuts, file dialogs, collapsible panel |
| `web/src/lib.rs` | The editor for the browser (WebAssembly), like `bridge.rs` |
| `web/www/canvas.js`, `panel.js`, `app.js` | The browser's canvas, panel and toolbar, ported from the QML |
| `core/src/netbox.rs` | Diagrams from NetBox sites, and refreshing them |
| `netbox-plugin/` | The NetBox plugin: the Diagram tab, saving, the site's data |

The core has no Qt dependency and is covered by unit tests (`cargo test` runs them).
QML (and the browser's JavaScript) only converts mouse positions to world coordinates and paints what Rust sends.

## Notes

- UI preferences (panel visibility, theme, layout options) are stored by Qt: in `~/.config/stratagram.conf`
  on Linux, in the registry (`HKEY_CURRENT_USER\Software\stratagram`) on Windows, and in
  `~/Library/Preferences` on macOS.
- With Qt's built-in (non-native) save dialog, press Enter in the file name field
  before the Save button becomes active. Native dialogs (GNOME, KDE, portals) are unaffected.

## License

Copyright 2026 Shin Sterneck. Licensed under the [Apache License, Version
2.0](LICENSE): use, change and share it freely, also commercially. Copies
and builds you pass on, changed or not, keep the license and the
[NOTICE](NOTICE) file, which credits the original work.

Stratagram is built with open-source libraries under their own licenses
(MIT, Apache-2.0, Zlib, Unicode and others): `cargo about generate -c
about.toml about.hbs` lists them with their license texts, and the NetBox
plugin's package carries that list as `THIRD-PARTY-LICENSES.txt`. The desktop
app uses Qt, under the GNU LGPL v3, linked dynamically.

### Trademarks

Product and company names such as Cisco, NetBox, Amazon Web Services (AWS),
Microsoft Azure, Google Cloud, Alibaba Cloud, Deutsche Telekom (Telekom
Cloud), STACKIT, Zscaler and draw.io are trademarks of their respective
owners. Stratagram uses them only to name the products and services a
diagram shows. The cloud provider tiles are simple stand-ins (a letter or
short name in the provider's color), not the providers' logos, and the
router symbol is Stratagram's own drawing in the style of common network
diagram symbols. Stratagram is not affiliated with or endorsed by any of
them.
