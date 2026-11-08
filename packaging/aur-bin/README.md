# AUR: stratagram-bin

`PKGBUILD` installs the prebuilt Linux download of a release
(`stratagram-X.Y.Z-linux-x86_64.tar.gz`, made by `packaging/linux/bundle.sh` on Arch Linux in
the Build workflow) as the [AUR package stratagram-bin](https://aur.archlinux.org/packages/stratagram-bin).
It provides and conflicts with `stratagram`, the package built from source (`packaging/aur`).

The Build workflow checks it on every push (`check.sh`: makepkg from the local file, installed
and run). For a new release, once the release has the Linux download: set `pkgver` (and
`pkgrel=1`), then

```sh
cd packaging/aur-bin
updpkgsums
makepkg -f
makepkg --printsrcinfo > .SRCINFO
```

and commit `PKGBUILD` and `.SRCINFO` to the AUR repository
(`ssh://aur@aur.archlinux.org/stratagram-bin.git`).
