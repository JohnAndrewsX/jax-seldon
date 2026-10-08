#!/usr/bin/env python3
"""WP-102b: tests/plugin/desk-view.sh cut to its helpers and the import
scenarios, written to <dir>/desk-import.sh (for plugin-mutants.py). Run from
the checkout: python3 work/active/WP-102/mk-desk-import.py <dir>."""
import os
import sys
S=sys.argv[1]
lines=open('tests/plugin/desk-view.sh').read().split('\n')
def idx(pred, start=0):
    for i in range(start,len(lines)):
        if pred(lines[i]): return i
    raise Exception("not found")
w=idx(lambda l: l.startswith('# 1. Width'))
head=lines[:w-1]
a=idx(lambda l: l.startswith("expected_warnings='jax\\.seldon: seldon (rules exit 1: AGENTS"), w)
b=idx(lambda l: l.startswith("wk='"), a)
m=idx(lambda l: l.startswith('# WP-102b, Import tasks'))
e=idx(lambda l: l.startswith('mkdir -p "$work/home-work-locked"'), m)
tail=['for h in home-import home-import-refused home-import-area home-import-reask home-import-hidden; do [[ -f "$work/$h/argv.log" ]] && cp "$work/$h/argv.log" "$OUT_DIR/$h.argv"; done', 'for c in import-live import-refused import-dev import-area import-reask import-hidden; do [[ -f "$work/$c.steps" ]] && { nl -ba "$work/$c.steps" > "$OUT_DIR/$c.steps"; cp "$work/$c.log" "$OUT_DIR/$c.log"; }; done',
      'echo "pass $pass fail $fail"']
head=[('root=' + os.getcwd()) if l.startswith('root=$(cd') else l for l in head]
open(S+'/desk-import.sh','w').write('\n'.join(head+lines[a:b+1]+lines[m:e]+tail)+'\n')
