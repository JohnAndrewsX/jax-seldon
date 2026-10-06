# Snapshots

Read this before the first red change of an R2 or R3 case.

Start the case first; then take the snapshot yourself, for each config that
`snapper --csvout list-configs` lists:

```bash
pkexec snapper -c <config> create -c number -p -d "<ID>"
```

`-p` prints the number. The description is the case id only: no logbook
text in the command. A command you run through your tool has no terminal
the user sees, so it is `pkexec` (Omarchy's rule); each config's command
asks for the password once more. Use `sudo` in place of `pkexec` only where
your command runs in the user's own terminal.

Not `omarchy-snapshot create` (`omarchy snapshot create`): its cleanup pass
prunes old numbered snapshots, and with Omarchy's limit of five that can be
the rollback of another case.

Record the `root` config's number as the case's rollback:

```bash
seldon plan snapshot <ID> <N> --actor agent:<name>
```

The engine checks that the snapshot exists and was taken after the case
started and before its first red change; it warns, never refuses. The other
configs' numbers go into a *Log* line `snapshot <N> (<config>) before <step>`.

## Without Snapper

No `snapper`, or no configs:

- **R3**: stop and ask the user.
- **R2**: take a named backup instead (a copy of the files the step changes,
  a list of the packages it replaces), name it in the *Plan* and say so in
  the *Log*.

## Retention

Numbered snapshots are pruned by the next cleanup (`omarchy update`,
`omarchy-snapshot create`). When a case's snapshot is pruned, Seldon writes
`rollback for <ID> pruned` into that case's *Log*. Nothing for you to do.
