# Flatpak

`app.stratagram.Stratagram.yml` builds Stratagram for [Flathub](https://flathub.org) on the KDE
runtime (which brings Qt), without network access: the Rust crates come from
`cargo-sources.json`, made from `Cargo.lock` by `cargo-sources.py`.

For a new release: set `tag` and `commit` in the manifest, and if `Cargo.lock` changed,

```sh
packaging/flatpak/cargo-sources.py > packaging/flatpak/cargo-sources.json
```

Then copy both files into the Flathub repository (`flathub/app.stratagram.Stratagram`).

To build and try it locally (needs `org.flatpak.Builder`, `org.kde.Sdk//6.11` and
`org.freedesktop.Sdk.Extension.rust-stable//25.08` from Flathub):

```sh
cd packaging/flatpak
flatpak run org.flatpak.Builder --user --install --force-clean build app.stratagram.Stratagram.yml
flatpak run app.stratagram.Stratagram
flatpak run --command=flatpak-builder-lint org.flatpak.Builder manifest app.stratagram.Stratagram.yml
```
