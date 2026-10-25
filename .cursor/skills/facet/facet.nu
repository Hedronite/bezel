#!/usr/bin/env nu
# Fixed read-only body for the facet skill. Wraps `facet` with `--json`
# after the subcommand.
# stdout is one JSON envelope: {ok, command, exit, data, error}
# Never calls history --bodies, history --id, blob, diff --bodies, or request run.

const BLOCKED = ["--bodies" "--id" "blob" "--dry-run"]
const OWN_FLAGS = ["--limit" "--request" "--status" "--session" "--open" "--environment"]

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

def opt [flag: string, v: string] {
  if $v == "" { [] } else { [$flag $v] }
}

def pos [v: string] {
  if $v == "" { [] } else { [$v] }
}

def blocked-argv [args: list<string>] {
  let joined = ($args | str join " ")
  ($args | any {|a| $a in $BLOCKED }) or ($joined | str contains "request run") or ($joined | str contains "request --dry-run")
}

def ascii-lower [text: string] {
  let pairs = [
    [A a] [B b] [C c] [D d] [E e] [F f] [G g] [H h] [I i] [J j]
    [K k] [L l] [M m] [N n] [O o] [P p] [Q q] [R r] [S s] [T t]
    [U u] [V v] [W w] [X x] [Y y] [Z z]
  ]
  $pairs | reduce --fold $text {|p, acc|
    $acc | str replace --all ($p | get 0) ($p | get 1)
  }
}

def secret-key [name: string] {
  let lowered = (ascii-lower $name)
  $lowered in [
    value values secret token password authorization
    header headers body bodies raw content cookie cookies
    request_body response_body
  ]
}

def env-name-key [name: string] {
  let chars = ($name | split chars)
  if ($chars | is-empty) {
    false
  } else {
    $chars | all {|c| $c =~ "[A-Z0-9_]" }
  }
}

def redact [v: any] {
  let d = ($v | describe)
  if ($d | str starts-with "list") or ($d | str starts-with "table") {
    $v | each {|item| redact $item }
  } else if ($d | str starts-with "record") {
    let cols = ($v | columns | where {|c| not (secret-key $c) })
    if ($cols | is-empty) {
      {}
    } else {
      $cols | reduce --fold {} {|c, acc|
        let val = ($v | get $c)
        let vd = ($val | describe)
        if (env-name-key $c) and $vd == "string" {
          $acc | upsert $c null
        } else {
          $acc | upsert $c (redact $val)
        }
      }
    }
  } else {
    $v
  }
}

def with-same [parsed: any, same: bool] {
  let d = ($parsed | describe)
  if ($d | str starts-with "record") {
    $parsed | merge { same: $same }
  } else {
    { same: $same, report: $parsed }
  }
}

def run-facet [command: string, args: list<string>, ok_codes: list<int> = [0], redact_values: bool = false, embed_same: bool = false] {
  let stray = ($args | where {|a| ($a | str starts-with "-") and (not ($a in $OWN_FLAGS)) })
  if (blocked-argv $args) or (not ($stray | is-empty)) {
    err-row $command 2 "refused: history bodies, blob, diff --bodies, and request run are not available from this skill"
  } else {
    let r = (try {
      ^facet ...$args --json | complete
    } catch {|e|
      { stdout: "", stderr: $"facet: ($e.msg)", exit_code: 127 }
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
        let data = if $redact_values { redact $parsed.value } else { $parsed.value }
        let out = if $embed_same { with-same $data ($r.exit_code == 0) } else { $data }
        ok-row $command $r.exit_code $out
      }
    }
  }
}

def "main history" [
  path: string = "",
  --limit: int = 20,
  --request: string = "",
  --status: string = "",
  --session: string = "",
  --bodies,
  --id: string = "",
  ...rest: string,
] {
  if $bodies or $id != "" or not ($rest | is-empty) or (blocked-argv (pos $path)) {
    err-row "history" 2 "refused: history bodies are not available from this skill" | to json --raw
  } else if $limit < 1 {
    err-row "history" 2 "limit must be >= 1" | to json --raw
  } else {
    let args = (
      ["history"]
      ++ (pos $path)
      ++ ["--limit" ($limit | into string)]
      ++ (opt "--request" $request)
      ++ (opt "--status" $status)
      ++ (opt "--session" $session)
    )
    run-facet "history" $args | to json --raw
  }
}

def "main last" [path: string = "", --request: string = "", --bodies, ...rest: string] {
  if $bodies or not ($rest | is-empty) or (blocked-argv (pos $path)) {
    err-row "last" 2 "refused: history bodies are not available from this skill" | to json --raw
  } else {
    run-facet "last" (["last"] ++ (pos $path) ++ (opt "--request" $request)) | to json --raw
  }
}

def "main sessions" [--open, ...rest: string] {
  if not ($rest | is-empty) {
    err-row "sessions" 2 "refused: facet skill is read-only" | to json --raw
  } else {
    let args = (["session" "list"] ++ (if $open { ["--open"] } else { [] }))
    run-facet "sessions" $args | to json --raw
  }
}

def "main session" [id: string, ...rest: string] {
  if not ($rest | is-empty) or ($id in ["start" "end" "delete" "rm"]) {
    err-row "session" 2 "refused: session start/end is not available from this skill" | to json --raw
  } else {
    run-facet "session" ["session" "show" $id] | to json --raw
  }
}

def "main pins" [path: string = "", ...rest: string] {
  if not ($rest | is-empty) or (blocked-argv (pos $path)) {
    err-row "pins" 2 "refused: facet skill is read-only" | to json --raw
  } else {
    run-facet "pins" (["pin" "list"] ++ (pos $path)) | to json --raw
  }
}

def "main env" [path: string = "", --environment: string = "", ...rest: string] {
  if not ($rest | is-empty) or ($path in ["set" "delete" "unset" "get" "add" "rm"]) {
    err-row "env" 2 "refused: env values and env writes are not available from this skill" | to json --raw
  } else {
    run-facet "env" (["env" "list"] ++ (pos $path) ++ (opt "--environment" $environment)) [0] true | to json --raw
  }
}

def "main diff" [a: string, b: string, path: string = "", --bodies, ...rest: string] {
  if $bodies or not ($rest | is-empty) or (blocked-argv [$a $b $path]) {
    err-row "diff" 2 "refused: diff --bodies is not available from this skill" | to json --raw
  } else {
    # exit 1 means the runs differ; that is a result, not a failure
    run-facet "diff" (["diff" $a $b] ++ (pos $path)) [0 1] false true | to json --raw
  }
}

def "main doctor" [...rest: string] {
  if not ($rest | is-empty) {
    err-row "doctor" 2 "refused: facet skill is read-only" | to json --raw
  } else {
    run-facet "doctor" ["doctor"] | to json --raw
  }
}

def "main blob" [...args: string] {
  err-row "blob" 2 "refused: blob is not available from this skill" | to json --raw
}

def "main request" [--dry-run, ...args: string] {
  err-row "request" 2 "refused: request run is not available from this skill" | to json --raw
}

def "main replay" [...args: string] {
  err-row "replay" 2 "refused: replay is not available from this skill" | to json --raw
}

def main [...args: string] {
  let command = ($args | first | default "")
  err-row $command 2 "refused: facet skill is read-only" | to json --raw
}
