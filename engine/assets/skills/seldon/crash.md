# After a Crash Analysis

Read this when you have diagnosed a crash with Omarchy's `diagnose-crash`
skill and there is a logbook.

File your report in the logbook's inbox, then ask the user in one line
whether it becomes a case (in an unattended session, file it and ask
nothing):

```bash
seldon inbox add --title "<title>" --tag crash --actor agent:<name> --file - <<'SELDON_REPORT'
<your report>
SELDON_REPORT
```

The quoted heredoc expands nothing; if a line of the report is
`SELDON_REPORT`, use another word in both places. Seldon redacts the report
and files it once, as `inbox/<date>-<slug>.md`. Never put the core, memory
contents or the process environment into it, nor a backtrace with variable
values (`bt`, not `bt full`). Filing changes nothing on the
machine and needs no case; when the user says yes, open one
([`case.md`](case.md)) and name the inbox file in its *Intent*.
