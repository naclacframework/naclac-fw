#!/usr/bin/env python3
"""Structural diff of two IDL JSON files: the AST-walker IDL and the compiled `idl-build` IDL.

List items that all carry a unique string `name` are matched by name, so a difference in order is
reported as ORDER and never hides a content difference. Everything else is compared in place.

A difference matching an entry of KNOWN (path regex, reason) is labelled KNOWN and does not fail the
run; any other difference is NEW and makes the exit code non-zero. KNOWN is empty: the two IDL
generators are expected to agree on everything the fixtures cover.

usage: diff_idl.py AST.json IB.json [--max-value-chars N]
"""

import json
import re
import sys

KNOWN = []

MISSING_IN_IB = "ONLY-IN-AST"
MISSING_IN_AST = "ONLY-IN-IDL-BUILD"
VALUE = "VALUE"
ORDER = "ORDER"
TYPE = "TYPE"


def short(value, limit):
    text = json.dumps(value, sort_keys=True)
    return text if len(text) <= limit else text[: limit - 3] + "..."


def name_key(items):
    if not items or not all(isinstance(i, dict) and isinstance(i.get("name"), str) for i in items):
        return None
    names = [i["name"] for i in items]
    return "name" if len(set(names)) == len(names) else None


def walk(ast, ib, path, out):
    if type(ast) is not type(ib):
        out.append((path, TYPE, ast, ib))
        return
    if isinstance(ast, dict):
        for key in sorted(set(ast) | set(ib)):
            child = f"{path}.{key}" if path else key
            if key not in ib:
                out.append((child, MISSING_IN_IB, ast[key], None))
            elif key not in ast:
                out.append((child, MISSING_IN_AST, None, ib[key]))
            else:
                walk(ast[key], ib[key], child, out)
    elif isinstance(ast, list):
        key = name_key(ast) if name_key(ast) and name_key(ib) else None
        if key:
            ast_names = [i[key] for i in ast]
            ib_names = [i[key] for i in ib]
            ast_map = {i[key]: i for i in ast}
            ib_map = {i[key]: i for i in ib}
            common_ast = [n for n in ast_names if n in ib_map]
            common_ib = [n for n in ib_names if n in ast_map]
            if common_ast != common_ib:
                out.append((path, ORDER, common_ast, common_ib))
            for name in ast_names:
                child = f"{path}[{name}]"
                if name not in ib_map:
                    out.append((child, MISSING_IN_IB, ast_map[name], None))
                else:
                    walk(ast_map[name], ib_map[name], child, out)
            for name in ib_names:
                if name not in ast_map:
                    out.append((f"{path}[{name}]", MISSING_IN_AST, None, ib_map[name]))
        else:
            for index in range(max(len(ast), len(ib))):
                child = f"{path}[{index}]"
                if index >= len(ib):
                    out.append((child, MISSING_IN_IB, ast[index], None))
                elif index >= len(ast):
                    out.append((child, MISSING_IN_AST, None, ib[index]))
                else:
                    walk(ast[index], ib[index], child, out)
    elif ast != ib:
        out.append((path, VALUE, ast, ib))


def known_reason(path):
    for pattern, reason in KNOWN:
        if pattern.match(path):
            return reason
    return None


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    limit = 160
    if "--max-value-chars" in sys.argv:
        limit = int(sys.argv[sys.argv.index("--max-value-chars") + 1])
        args = [a for a in args if a != str(limit)]
    if len(args) != 2:
        print(__doc__)
        return 2
    with open(args[0], encoding="utf-8") as f:
        ast = json.load(f)
    with open(args[1], encoding="utf-8") as f:
        ib = json.load(f)

    diffs = []
    walk(ast, ib, "", diffs)

    new = [d for d in diffs if not known_reason(d[0])]
    known = [d for d in diffs if known_reason(d[0])]

    for path, kind, a, b in new:
        print(f"  NEW   {kind:<18} {path}")
        if kind == MISSING_IN_IB:
            print(f"        ast: {short(a, limit)}")
        elif kind == MISSING_IN_AST:
            print(f"        idl-build: {short(b, limit)}")
        else:
            print(f"        ast:       {short(a, limit)}")
            print(f"        idl-build: {short(b, limit)}")
    for path, kind, a, b in known:
        print(f"  KNOWN {kind:<18} {path}   [{known_reason(path)}]")

    print(f"  -> {len(new)} new difference(s), {len(known)} known")
    return 1 if new else 0


if __name__ == "__main__":
    sys.exit(main())
