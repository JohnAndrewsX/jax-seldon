# omarchy-pkgs recipe draft (not submitted)

`.omarchy/package.json` is the recipe's metadata for `omacom/omarchy-pkgs`
(`pkgbuilds/seldon/.omarchy/package.json`): `source: local`, `auto_merge`
and an `upstream.watch` on the GitHub repository, so a later release syncs on
its own.

The pull request to `omacom/omarchy-pkgs` is the operator's, after 0.5.0.
It carries, next to this file's `.omarchy/`, `PKGBUILD` and `seldon.install`
of `packaging/seldon/` (the desktop entry and the icon are read from the
tarball), with `pkgver` and `sha256sums` of the release asset
(`https://github.com/JohnAndrewsX/jax-seldon/releases/download/vX.Y.Z/jax-seldon-X.Y.Z.tar.gz`,
the file `SHA256SUMS` lists and the attestation covers). Nothing here is sent
anywhere by the repository's tooling.
