#!/usr/bin/env nu
# Fixed metadata-only body for the geode skill. Wraps `geode --output json`.
# stdout is one JSON envelope: {ok, command, exit, data, error}
# Only the allowlist is passed to geode. Anything else is refused.

const ALLOWED = [
  "list"
  "verify"
  "vault recipients"
  "snapshot ls"
  "policy show"
  "policy check"
  "audit verify"
  "custody status"
  "keyring list"
]

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

def refused [command: string, detail: string] {
  err-row $command 2 $"refused: geode skill is metadata-only ($detail)"
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

def vault-path [v: string] {
  if $v != "" { $v } else { $env.GEODE_VAULT? | default "" }
}

def run-geode [
  command: string,
  sub: list<string>,
  rest: list<string>,
  ok_codes: list<int> = [0],
  permit_flags: list<string> = [],
] {
  let key = ($sub | str join " ")
  let flagged = ($rest | where {|a| ($a | str starts-with "-") and (not ($a in $permit_flags)) })
  if not ($key in $ALLOWED) {
    refused $command $"not on the allowlist: ($key)"
  } else if not ($flagged | is-empty) {
    refused $command "flags are not accepted"
  } else {
    let argv = (["--output" "json"] ++ $sub ++ $rest)
    let r = (try {
      ^geode ...$argv | complete
    } catch {|e|
      { stdout: "", stderr: $"geode: ($e.msg)", exit_code: 127 }
    })
    let stdout = (as-text $r.stdout)
    let stderr = (as-text $r.stderr)
    if not ($r.exit_code in $ok_codes) {
      err-row $command $r.exit_code (clip-error $stderr $stdout)
    } else {
      let parsed = (try { { ok: true, value: (parse-json $stdout) } } catch {|e| { ok: false, value: $e.msg } })
      if not $parsed.ok {
        err-row $command 1 $"invalid json: ($parsed.value)"
      } else {
        ok-row $command $r.exit_code $parsed.value
      }
    }
  }
}

def path-verb [p: string] {
  $p in [
    set init add-recipient rotate add unlock lock delete rm
    start end get unset cat open mount run agent keygen seal sign gc
  ]
}

def need-vault [command: string, vault: string] {
  let p = (vault-path $vault)
  if (path-verb $vault) {
    refused $command $"($vault) is not available from this skill"
  } else if $p == "" {
    err-row $command 2 "no vault: pass one or set GEODE_VAULT"
  } else if ($p | str starts-with "-") {
    refused $command "vault path must not be a flag"
  } else {
    { ok: true, path: $p }
  }
}

def "main list" [vault: string = "", prefix: string = "", ...rest: string] {
  if not ($rest | is-empty) {
    refused "list" "extra arguments" | to json --raw
  } else {
    let got = (need-vault "list" $vault)
    if not $got.ok {
      $got | to json --raw
    } else {
      let rest_args = ([$got.path] ++ (if $prefix == "" { [] } else { [$prefix] }))
      run-geode "list" [list] $rest_args | to json --raw
    }
  }
}

def "main verify" [vault: string = "", --sample: string = "", ...rest: string] {
  if not ($rest | is-empty) {
    refused "verify" "extra arguments" | to json --raw
  } else {
    let got = (need-vault "verify" $vault)
    if not $got.ok {
      $got | to json --raw
    } else if $sample == "" {
      run-geode "verify" [verify] [$got.path "--cheap"] [0] ["--cheap"] | to json --raw
    } else {
      let numeric = (try { $sample | into float; true } catch {|_| false })
      if not $numeric or ($sample | str starts-with "-") {
        refused "verify" "sample must be a non-negative number" | to json --raw
      } else {
        run-geode "verify" [verify] [$got.path "--sample" $sample] [0] ["--sample"] | to json --raw
      }
    }
  }
}

def "main recipients" [vault: string = "", ...rest: string] {
  if not ($rest | is-empty) {
    refused "recipients" "extra arguments" | to json --raw
  } else {
    let got = (need-vault "recipients" $vault)
    if not $got.ok { $got | to json --raw } else {
      run-geode "recipients" [vault recipients] [$got.path] | to json --raw
    }
  }
}

def "main snapshots" [vault: string = "", ...rest: string] {
  if not ($rest | is-empty) {
    refused "snapshots" "extra arguments" | to json --raw
  } else {
    let got = (need-vault "snapshots" $vault)
    if not $got.ok { $got | to json --raw } else {
      run-geode "snapshots" [snapshot ls] [$got.path] | to json --raw
    }
  }
}

def "main policy" [vault: string = "", ...rest: string] {
  if not ($rest | is-empty) {
    refused "policy" "policy set is not available from this skill" | to json --raw
  } else {
    let got = (need-vault "policy" $vault)
    if not $got.ok { $got | to json --raw } else {
      run-geode "policy" [policy show] [$got.path] | to json --raw
    }
  }
}

def "main can" [principal: string, op: string, path: string, vault: string = "", ...rest: string] {
  if not ($rest | is-empty) {
    refused "can" "extra arguments" | to json --raw
  } else if ([$principal $op $path] | any {|a| $a | str starts-with "-" }) {
    refused "can" "flags are not accepted" | to json --raw
  } else {
    let got = (need-vault "can" $vault)
    if not $got.ok { $got | to json --raw } else {
      # deny exits 3; that is a result, not a failure
      run-geode "can" [policy check] [$got.path $principal $op $path] [0 3] | to json --raw
    }
  }
}

def "main audit" [...rest: string] {
  if not ($rest | is-empty) {
    refused "audit" "extra arguments" | to json --raw
  } else {
    # a broken chain exits 2; that is a result, not a failure
    run-geode "audit" [audit verify] [] [0 2] | to json --raw
  }
}

def "main custody" [...rest: string] {
  if not ($rest | is-empty) {
    refused "custody" "unlock/lock is not available from this skill" | to json --raw
  } else {
    run-geode "custody" [custody status] [] | to json --raw
  }
}

def "main keyring" [...rest: string] {
  if not ($rest | is-empty) {
    refused "keyring" "keyring add is not available from this skill" | to json --raw
  } else {
    run-geode "keyring" [keyring list] [] | to json --raw
  }
}

def main [...args: string] {
  let command = ($args | first | default "")
  refused $command ($args | str join " ") | to json --raw
}
