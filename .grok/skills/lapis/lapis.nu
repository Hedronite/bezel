#!/usr/bin/env nu
# Fixed read-only body for the lapis skill. Wraps `lapis --json`.
# stdout is one JSON envelope: {ok, command, exit, data, error}
# Vault: --vault, else $LAPIS_VAULT, else the lapis config default.

def clip-error [stderr: string, stdout: string] {
  let joined = ([$stderr $stdout] | str join " " | str trim)
  if ($joined | is-empty) {
    "command failed"
  } else {
    ($joined | lines | first | str substring 0..399)
  }
}

def ok-row [command: string, exit: int, data: any] {
  { ok: true, command: $command, exit: $exit, data: $data, error: null }
}

def err-row [command: string, exit: int, error: string] {
  { ok: false, command: $command, exit: $exit, data: null, error: ($error | str substring 0..399) }
}

def as-text [raw: any] {
  let d = ($raw | describe)
  if $d == "string" {
    $raw
  } else {
    try { $raw | decode utf-8 } catch { "" }
  }
}

def parse-json [text: string] {
  let t = ($text | str trim)
  if ($t | is-empty) {
    error make { msg: "empty json" }
  }
  $t | from json --strict
}

def vault-args [vault: string] {
  let v = if $vault != "" { $vault } else { $env.LAPIS_VAULT? | default "" }
  if ($v | str starts-with "-") {
    error make { msg: "vault path must not be a flag" }
  }
  if $v == "" { [] } else { ["--vault" $v] }
}

def lapis-data [value: any] {
  let d = ($value | describe)
  if ($d | str starts-with "record") and ("data" in ($value | columns)) {
    $value | get data
  } else {
    $value
  }
}

def run-lapis [command: string, args: list<string>, vault: string] {
  let vault_part = (try { vault-args $vault } catch {|e| $e.msg })
  if ($vault_part | describe) == "string" {
    err-row $command 2 $vault_part
  } else {
    let argv = (["--json"] ++ $vault_part ++ $args)
    let r = (try {
      ^lapis ...$argv | complete
    } catch {|e|
      { stdout: "", stderr: $"lapis: ($e.msg)", exit_code: 127 }
    })
    let stdout = (as-text $r.stdout)
    let stderr = (as-text $r.stderr)
    if $r.exit_code != 0 {
      err-row $command $r.exit_code (clip-error $stderr $stdout)
    } else {
      let parsed = (try { { ok: true, value: (parse-json $stdout) } } catch {|e| { ok: false, value: $e.msg } })
      if not $parsed.ok {
        err-row $command 1 $"invalid json: ($parsed.value)"
      } else {
        ok-row $command 0 (lapis-data $parsed.value)
      }
    }
  }
}

def flagish [s: string] { $s | str starts-with "-" }

def "main search" [query: string, --limit (-n): int = 5, --domain: string = "", --vault: string = ""] {
  if $limit < 1 {
    err-row "search" 2 "limit must be >= 1" | to json --raw
  } else if (flagish $query) or (flagish $domain) {
    err-row "search" 2 "refused: lapis skill is read-only" | to json --raw
  } else {
    mut args = ["search" $query "-n" ($limit | into string)]
    if $domain != "" { $args = ($args ++ ["--domain" $domain]) }
    let row = (run-lapis "search" $args $vault)
    if not $row.ok {
      $row | to json --raw
    } else {
      let hits = (try { $row.data | get hits } catch {|err| null })
      let kind = ($hits | describe)
      if not (($kind | str starts-with "list") or ($kind | str starts-with "table")) {
        err-row "search" 1 "search data has no hits" | to json --raw
      } else {
        let slim = (try { $hits | select path title } catch {|err| null })
        if ($slim | describe) == "nothing" {
          err-row "search" 1 "search hits missing path or title" | to json --raw
        } else {
          ok-row "search" $row.exit $slim | to json --raw
        }
      }
    }
  }
}

def "main read" [path: string, --vault: string = ""] {
  if (flagish $path) {
    err-row "read" 2 "refused: lapis skill is read-only" | to json --raw
  } else {
    run-lapis "read" ["read" $path] $vault | to json --raw
  }
}

def "main list" [--prefix: string = "", --vault: string = ""] {
  let row = (run-lapis "list" ["list"] $vault)
  if not $row.ok or $prefix == "" {
    $row | to json --raw
  } else {
    let kind = ($row.data | describe)
    if not (($kind | str starts-with "list") or ($kind | str starts-with "table")) {
      err-row "list" 1 "list data is not a list" | to json --raw
    } else {
      let filtered = ($row.data | where path starts-with $prefix)
      ok-row "list" $row.exit $filtered | to json --raw
    }
  }
}

def "main neighbors" [path: string, --vault: string = ""] {
  if (flagish $path) {
    err-row "neighbors" 2 "refused: lapis skill is read-only" | to json --raw
  } else {
    run-lapis "neighbors" ["neighbors" $path] $vault | to json --raw
  }
}

def "main resolve" [link: string, --vault: string = ""] {
  if (flagish $link) {
    err-row "resolve" 2 "refused: lapis skill is read-only" | to json --raw
  } else {
    run-lapis "resolve" ["resolve" $link] $vault | to json --raw
  }
}

# Lapis 0.4.2 rows are path, title, domain, priority, mtime_iso.
# Older builds are path, title, mtime. Read those fields by name.
def recent-mtime [row: record] {
  let cols = ($row | columns)
  if "mtime_iso" in $cols {
    $row | get mtime_iso
  } else if "mtime" in $cols {
    $row | get mtime
  } else {
    error make { msg: "recent row missing mtime" }
  }
}

def "main recent" [--limit (-n): int = 10, --vault: string = ""] {
  if $limit < 1 {
    err-row "recent" 2 "limit must be >= 1" | to json --raw
  } else {
    let row = (run-lapis "recent" ["analytics" "recent"] $vault)
    if not $row.ok {
      $row | to json --raw
    } else {
      let rows = (try { $row.data | get rows } catch {|err| null })
      let shaped = (try {
        $rows | first $limit | each {|r|
          {
            path: ($r | get path)
            title: ($r | get title)
            mtime: (recent-mtime $r)
          }
        }
      } catch {|err| null })
      if ($shaped | describe) == "nothing" {
        err-row "recent" 1 "recent data has no rows" | to json --raw
      } else {
        ok-row "recent" $row.exit $shaped | to json --raw
      }
    }
  }
}

def "main status" [--vault: string = ""] {
  run-lapis "status" ["vault" "info"] $vault | to json --raw
}

def "main doctor" [--vault: string = ""] {
  run-lapis "doctor" ["doctor"] $vault | to json --raw
}

def main [...args: string] {
  let command = ($args | first | default "")
  err-row $command 2 "refused: lapis skill is read-only" | to json --raw
}
