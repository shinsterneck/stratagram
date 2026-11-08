# AUR

`PKGBUILD` builds the `stratagram` package for the
[Arch User Repository](https://aur.archlinux.org/packages/stratagram) from a release's source.

For a new release: set `pkgver` (and `pkgrel=1`), then

```sh
cd packaging/aur
updpkgsums                         # the release archive's checksum
makepkg -f                         # build and test it
makepkg --printsrcinfo > .SRCINFO
```

and commit `PKGBUILD` and `.SRCINFO` to the AUR repository
(`ssh://aur@aur.archlinux.org/stratagram.git`).
