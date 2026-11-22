#!/usr/bin/env bash
# Opt-in hashed private-name guard. The repository stores no plaintext names.
#
# Generate a hash file (terms on stdin, one per line; blank and # lines skip):
#   printf '%s\n' name one two | scripts/check-private-hashes.sh --hash SALT
#
# Scan (adopter supplies the hash file; nothing is stored in the repo):
#   scripts/check-private-hashes.sh --hash-file hashes.json [paths...]
#   BEZEL_PRIVATE_HASHES=hashes.json scripts/check-private-hashes.sh [paths...]
#
# With no hash file configured it exits 0 (opt-in). A configured but missing,
# unreadable, or malformed hash file exits 2. A hit exits 1 and prints only
# the matched SHA-256 prefix, a line number or "path", and a relative path
# whose matching substrings are replaced by [REDACTED]; the plaintext term and
# the line content are never printed.
set -euo pipefail
python=${PYTHON:-python3}
code=$(cat <<'PY'
import hashlib
import json
import os
import re
import sys

PREFIX = "check-private-hashes"
TOKEN = re.compile(r"[a-z0-9._-]+")
DELIMS = re.compile(r"[._-]+")
HEX = re.compile(r"^[0-9a-f]{64}$")


def usage(message):
    print(f"{PREFIX}: {message}", file=sys.stderr)
    print(
        "usage: check-private-hashes.sh --hash SALT | "
        "[--hash-file PATH] [paths...]",
        file=sys.stderr,
    )
    raise SystemExit(2)


def generator(salt):
    if salt == "":
        usage("salt must not be empty")
    terms = []
    for line in sys.stdin:
        term = line.strip().lower()
        if not term or term.startswith("#"):
            continue
        if term not in terms:
            terms.append(term)
    if not terms:
        usage("no terms on stdin")
    hashes = [hashlib.sha256((salt + "\0" + term).encode("utf-8")).hexdigest() for term in terms]
    document = {
        "salt": salt,
        "lengths": sorted({len(term) for term in terms}),
        "hashes": sorted(hashes),
    }
    print(json.dumps(document, sort_keys=True))
    raise SystemExit(0)


def load(path):
    try:
        with open(path, encoding="utf-8") as handle:
            document = json.load(handle)
    except OSError as error:
        print(f"{PREFIX}: cannot read hash file: {error.strerror or error}", file=sys.stderr)
        raise SystemExit(2)
    except (UnicodeDecodeError, json.JSONDecodeError):
        print(f"{PREFIX}: hash file is not valid JSON", file=sys.stderr)
        raise SystemExit(2)
    if not isinstance(document, dict):
        print(f"{PREFIX}: hash file is not a JSON object", file=sys.stderr)
        raise SystemExit(2)
    salt = document.get("salt")
    if not isinstance(salt, str) or salt == "":
        print(f"{PREFIX}: hash file salt is missing or empty", file=sys.stderr)
        raise SystemExit(2)
    hashes = document.get("hashes")
    if (
        not isinstance(hashes, list)
        or not hashes
        or not all(isinstance(item, str) and HEX.match(item.lower()) for item in hashes)
    ):
        print(f"{PREFIX}: hash file hashes must be a non-empty list of sha256 hex strings", file=sys.stderr)
        raise SystemExit(2)
    hashes = [item.lower() for item in hashes]
    lengths = document.get("lengths")
    if lengths is None:
        lengths = list(range(3, 33))
    elif (
        not isinstance(lengths, list)
        or not lengths
        or not all(isinstance(item, int) and 1 <= item <= 128 for item in lengths)
    ):
        print(f"{PREFIX}: hash file lengths must be a non-empty list of integers 1..128", file=sys.stderr)
        raise SystemExit(2)
    return salt, set(hashes), sorted(set(lengths), reverse=True)


def digest(salt, text):
    return hashlib.sha256((salt + "\0" + text).encode("utf-8")).hexdigest()


def candidates(text, lengths):
    for token in TOKEN.findall(text):
        yield token
        for part in DELIMS.split(token):
            if part:
                yield part
        for size in lengths:
            if size <= len(token):
                for start in range(len(token) - size + 1):
                    yield token[start : start + size]


def mask_segment(segment, salt, hashes, lengths):
    out = []
    index = 0
    while index < len(segment):
        matched = 0
        for size in lengths:
            if index + size <= len(segment):
                if digest(salt, segment[index : index + size]) in hashes:
                    matched = size
                    break
        if matched:
            out.append("[REDACTED]")
            index += matched
        else:
            out.append(segment[index])
            index += 1
    return "".join(out)


def mask_path(rel, salt, hashes, lengths):
    parts = rel.replace(os.sep, "/").split("/")
    return "/".join(mask_segment(part, salt, hashes, lengths) for part in parts)


def hit_hash(text, salt, hashes, lengths):
    for candidate in candidates(text, lengths):
        value = digest(salt, candidate)
        if value in hashes:
            return value
    return None


def scan_file(path, rel, salt, hashes, lengths, report):
    try:
        with open(path, "rb") as handle:
            raw = handle.read()
    except OSError:
        return
    if b"\0" in raw:
        return
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError:
        return
    masked = mask_path(rel, salt, hashes, lengths)
    path_hit = hit_hash(rel.lower(), salt, hashes, lengths)
    if path_hit is not None:
        report(f"sha256:{path_hit[:12]} path {masked}")
        return
    for number, line in enumerate(text.splitlines(), 1):
        value = hit_hash(line.lower(), salt, hashes, lengths)
        if value is not None:
            report(f"sha256:{value[:12]} line {number} {masked}")
            return


def scan(paths, salt, hashes, lengths, report):
    for target in paths:
        if os.path.isdir(target):
            base = os.path.abspath(target)
            for current, dirs, files in os.walk(target):
                dirs[:] = sorted(d for d in dirs if d != ".git")
                for name in sorted(files):
                    full = os.path.join(current, name)
                    if os.path.islink(full):
                        continue
                    rel = os.path.relpath(full, base)
                    scan_file(full, rel, salt, hashes, lengths, report)
        elif os.path.isfile(target):
            scan_file(target, os.path.basename(target), salt, hashes, lengths, report)
        else:
            print(f"{PREFIX}: path not found: {target}", file=sys.stderr)
            raise SystemExit(2)


def main(argv):
    rest = list(argv)
    salt = None
    hash_file = None
    paths = []
    while rest:
        arg = rest.pop(0)
        if arg == "--hash":
            if not rest:
                usage("--hash needs a salt")
            salt = rest.pop(0)
        elif arg.startswith("--hash="):
            salt = arg.split("=", 1)[1]
        elif arg == "--hash-file":
            if not rest:
                usage("--hash-file needs a path")
            hash_file = rest.pop(0)
        elif arg.startswith("--hash-file="):
            hash_file = arg.split("=", 1)[1]
        elif arg == "--":
            paths.extend(rest)
            rest = []
        elif arg.startswith("-") and arg != "-":
            usage(f"unknown option: {arg}")
        else:
            paths.append(arg)
    if salt is not None:
        if hash_file is not None or paths:
            usage("--hash is generator mode; it takes no paths")
        generator(salt)
    if hash_file is None:
        hash_file = os.environ.get("BEZEL_PRIVATE_HASHES") or None
    if hash_file is None:
        raise SystemExit(0)
    salt_value, hashes, lengths = load(hash_file)
    hits = []
    scan(paths or ["."], salt_value, hashes, lengths, hits.append)
    if hits:
        for line in hits:
            print(line)
        raise SystemExit(1)
    raise SystemExit(0)


try:
    main(sys.argv[1:])
except SystemExit:
    raise
except Exception as error:  # never print a traceback for a guard failure
    print(f"{PREFIX}: {type(error).__name__}: {error}", file=sys.stderr)
    raise SystemExit(2)
PY
)
exec "$python" -B -c "$code" "$@"
