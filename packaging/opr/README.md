# omarchy-pkgs recipe draft (not submitted)

`.omarchy/package.json` is the recipe's metadata for `omacom/omarchy-pkgs`
(`pkgbuilds/seldon/.omarchy/package.json`): `source: local`, `auto_merge`
and an `upstream.watch` on the GitHub repository, so a later release syncs on
its own.

The pull request to `omacom/omarchy-pkgs` is the operator's. It carries, next
to this file's `.omarchy/`, the files of `packaging/seldon/`:
`PKGBUILD`, `seldon.install`, `seldon.desktop` and `seldon.svg`, with
`pkgver` and `sha256sums` of the release tarball
(`https://github.com/JohnAndrewsX/jax-seldon/archive/refs/tags/vX.Y.Z.tar.gz`).
Nothing here is sent anywhere by the repository's tooling.
