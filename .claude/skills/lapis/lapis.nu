#!/usr/bin/env nu
# Deterministic body for the lapis skill. Wraps `lapis --json`. Reads the vault; `reindex`
# writes the index, never the vault.
# Vault: --vault flag, else $LAPIS_VAULT, else the lapis config default.
# Envelope: { ok, command, data, meta? } or { ok: false, command, error: { kind, message, exit } }.

# No value may be read by lapis as a flag, even one that starts with `-` (a query or
# path copied from a note or the web). Positional values go after `--`; an option's
# value is attached to it as `--name=value`.
def vault-args [vault: string] {
  let v = if $vault != "" { $vault } else { $env.LAPIS_VAULT? | default "" }
  if $v == "" { [] } else { [$"--vault=($v)"] }
}

# Run `lapis --json …`. The CLI prints its own JSON envelope on stdout, success or not;
# keep its data and meta, and its error as fields an agent can branch on.
def envelope [command: string, args: list<string>, vault: string] {
  let r = (^lapis --json ...(vault-args $vault) ...$args | complete)
  let parsed = (try { $r.stdout | from json } catch { null })
  if $r.exit_code == 0 and $parsed != null {
    let meta = ($parsed | get -o meta | default {} | reject -o latency_ms)
    # meta rides along when there is more to page, or when the index lags the disk
    # (`stale`: notes newer than their index entry or missing from it; newer lapis only).
    if ($meta | get -o truncated | default false) or (($meta | get -o stale | default 0) > 0) {
      { ok: true, command: $command, data: ($parsed | get -o data), meta: $meta }
    } else {
      { ok: true, command: $command, data: ($parsed | get -o data) }
    }
  } else {
    let cli = if $parsed != null { $parsed | get -o error } else { null }
    let error = if $cli != null {
      { kind: ($cli | get -o kind | default "error"), message: ($cli | get -o message | default ""), exit: ($cli | get -o exit | default $r.exit_code) }
    } else {
      { kind: "error", message: ([$r.stderr $r.stdout] | str join " " | str trim | str substring 0..<400), exit: $r.exit_code }
    }
    { ok: false, command: $command, error: $error }
  }
}

# Print an envelope; a failure exits with the CLI's exit code.
def finish [e: record] {
  print ($e | to json)
  if not $e.ok { exit ($e.error.exit? | default 1) }
}

# Note paths: vault-relative pass through; absolute (or ~) paths under the vault
# root become vault-relative; absolute paths outside the vault fail closed.
def vault-root [vault: string] {
  let v = if $vault != "" { $vault } else { $env.LAPIS_VAULT? | default "" }
  let raw = if $v != "" { $v } else {
    let r = (^lapis --json vault info | complete)
    if $r.exit_code == 0 { $r.stdout | from json | get -o data.vaultRoot | default "" } else { "" }
  }
  if $raw == "" { "" } else { $raw | path expand | str trim -r -c "/" }
}

def to-vault-path [p: string, vault: string] {
  let q = if ($p | str starts-with "~") { $p | path expand } else { $p }
  if not ($q | str starts-with "/") { return { ok: true, path: $p } }
  let root = (vault-root $vault)
  if $root == "" { return { ok: false, error: "absolute path given but vault root could not be resolved" } }
  let abs = ($q | path expand | str trim -r -c "/")
  if ($abs | str starts-with $"($root)/") {
    { ok: true, path: ($abs | str substring (($root | str length) + 1)..) }
  } else {
    { ok: false, error: $"path outside vault root ($root): ($p)" }
  }
}

def path-envelope [command: string, path: string, vault: string] {
  let t = (to-vault-path $path $vault)
  if not $t.ok {
    { ok: false, command: $command, error: { kind: "sandbox", message: $t.error, exit: 2 } }
  } else {
    envelope $command [$command "--" $t.path] $vault
  }
}

# --mode bm25 (default) skips the vector arm and never contacts the embedder: about
# 0.2 s against 5 s for hybrid, usually with the same top hits. hybrid and vector opt in.
def "main search" [
  query: string
  --limit (-n): int = 5
  --domain: string = ""
  --mode: string = "bm25"
  --vault: string = ""
] {
  if $mode not-in ["bm25" "hybrid" "vector"] {
    finish { ok: false, command: "search", error: { kind: "bad_args", message: "--mode is bm25, hybrid or vector", exit: 1 } }
    return
  }
  mut a = ["search" "-n" ($limit | into string) "--per-doc"]
  $a = ($a | append (if $mode == "bm25" { ["--embedder" "none"] } else { ["--mode" $mode] }))
  if $domain != "" { $a = ($a | append $"--domain=($domain)") }
  $a = ($a | append ["--" $query])
  let e = (envelope "search" $a $vault)
  if not $e.ok { finish $e; return }
  let hits = ($e.data.hits? | default [] | each {|h|
    let snippet = ($h.snippet? | default "" | str replace -ra '\s+' ' ' | str trim)
    {
      path: $h.path
      title: ($h.title? | default null)
      heading: ($h.heading? | default null)
      score: ($h.score? | default null)
      snippet: (if ($snippet | str length) > 200 { ($snippet | str substring 0..<200) + "…" } else { $snippet })
    }
  })
  finish ($e | update data $hits)
}

# --section returns the part under the first heading whose text contains the value
# (case-insensitive), up to the next heading of the same or a higher level.
# --max-bytes caps the body; body_bytes and body_truncated report what was cut.
def "main read" [path: string, --section: string = "", --max-bytes: int = 0, --vault: string = ""] {
  let e = (path-envelope "read" $path $vault)
  if not $e.ok { finish $e; return }
  mut body = ($e.data.body? | default "")
  if $section != "" {
    let lines = ($body | lines)
    let want = ($section | str lowercase)
    let found = ($lines | enumerate | where {|l| ($l.item =~ '^#{1,6} ') and ($l.item | str lowercase | str contains $want) } | first 1)
    if ($found | is-empty) {
      finish { ok: false, command: "read", error: { kind: "not_found", message: $"no heading contains '($section)'", exit: 3 } }
      return
    }
    let start = ($found | first | get index)
    let level = ($found | first | get item | parse -r '^(?<h>#+)' | get h.0 | str length)
    let rest = ($lines | skip ($start + 1) | enumerate | where {|l|
      ($l.item =~ '^#{1,6} ') and (($l.item | parse -r '^(?<h>#+)' | get h.0 | str length) <= $level)
    })
    let end = if ($rest | is-empty) { $lines | length } else { $start + 1 + ($rest | first | get index) }
    $body = ($lines | slice $start..<$end | str join "\n")
  }
  let full = ($body | str length --utf-8-bytes)
  let cut = $max_bytes > 0 and $full > $max_bytes
  let shown = if $cut { $body | str substring --utf-8-bytes 0..<$max_bytes } else { $body }
  finish ($e | update data {|x| $x.data | upsert body $shown | upsert body_bytes $full | upsert body_truncated $cut })
}

# Filtering happens in the index: the prefix goes to `lapis list`, so a folder past
# the first page of results is never reported empty. meta.truncated says there is more.
def "main list" [
  --prefix: string = ""
  --limit (-n): int = 200
  --offset: int = 0
  --vault: string = ""
] {
  mut a = ["list" "-n" ($limit | into string) "--offset" ($offset | into string)]
  if $prefix != "" { $a = ($a | append ["--" $prefix]) }
  finish (envelope "list" $a $vault)
}

def "main neighbors" [path: string, --vault: string = ""] { finish (path-envelope "neighbors" $path $vault) }

def "main resolve" [link: string, --vault: string = ""] { finish (envelope "resolve" ["resolve" "--" $link] $vault) }

def "main recent" [--limit (-n): int = 10, --vault: string = ""] {
  let e = (envelope "recent" ["analytics" "recent"] $vault)
  if not $e.ok { finish $e; return }
  # map rows by the returned column names; column order differs across lapis versions
  finish ($e | update data { |x|
    let cols = $x.data.columns
    $x.data.rows | first $limit | each { |r| $cols | zip $r | reduce -f {} { |p, acc| $acc | insert $p.0 $p.1 } }
  })
}

# --brief answers "is it up": vault root, lattice reachability, documents indexed.
def "main status" [--brief, --vault: string = ""] {
  let e = (envelope "status" ["vault" "info"] $vault)
  if not $e.ok or not $brief { finish $e; return }
  finish ($e | update data {|x| {
    vaultRoot: $x.data.vaultRoot
    lattice: $x.data.lattice.mode
    reachable: ($x.data.lattice.reachable? | default false)
    documents_indexed: ($x.data.lattice.health?.documents_indexed? | default null)
    stale: ($x.data.lattice.stale?.count? | default null)
    embedder: ($x.data.lattice.health?.embedder? | default null)
  } })
}

def "main doctor" [--vault: string = ""] { finish (envelope "doctor" ["doctor"] $vault) }

# Re-index one note after it was written by hand, so search can find it. Writes the
# index only, never the note; running it twice is harmless. Same path rules as `read`.
def "main reindex" [path: string, --vault: string = ""] { finish (path-envelope "reindex" $path $vault) }

def main [] { help main }
