#!/usr/bin/env nu
# Nickel -> Nushell pack materializer (proto-ncl-pack/v0).
# Conforms to Bezel's shared one-line JSON envelope:
#   { ok, command, exit, data, error }
# Writes bytes emitted by Nickel; renders nothing itself.

def emit [ok: bool, command: string, exit_code: int, data: any, error: any] {
  let doc = {
    ok: $ok
    command: $command
    exit: $exit_code
    data: $data
    error: $error
  }
  print ($doc | to json --raw)
}

def fail [command: string, code: int, msg: string] {
  emit false $command $code null $msg
}

def resolve-nickel [override: string] {
  if $override != "" {
    return $override
  }
  if ($env.NICKEL_BIN? | default "") != "" {
    return $env.NICKEL_BIN
  }
  "nickel"
}

# Parse CODE: detail lines from a list of text blocks.
def extract-coded-lines [texts: list<string>] {
  mut out = []
  for txt in $texts {
    for line in ($txt | lines | each {|l| $l | str trim }) {
      let m = ($line | parse -r '^([A-Z][A-Z0-9_]+):\s*(.+)$')
      if not ($m | is-empty) {
        let row = ($m | first)
        $out = ($out | append { code: $row.capture0, detail: ($row.capture1 | str trim) })
      }
    }
  }
  $out
}

# Parse Nickel `--error-format json` output into structured violations + raw blame.
def parse-nickel-diagnostics [raw_err: string] {
  let trimmed = ($raw_err | str trim)
  let parsed = (try { $trimmed | from json } catch { null })
  if $parsed == null {
    return {
      violations: [{ code: "NICKEL_ERROR", detail: (if $trimmed == "" { "nickel export failed with empty error" } else { $trimmed }) }]
      blame: $trimmed
    }
  }
  let diags = ($parsed | get -o diagnostics | default [])
  mut violations = []
  for d in $diags {
    let sev = ($d | get -o severity | default "Error")
    let msg = ($d | get -o message | default "")
    let notes = ($d | get -o notes | default [] | each {|n| $n | into string })
    let from_notes = (extract-coded-lines $notes)
    let from_msg = (extract-coded-lines [$msg])
    # If PACK_INVALID wraps concrete violations in notes[], emit the concrete notes[] codes directly.
    let has_wrapper = ($from_msg | any {|v| $v.code == "PACK_INVALID" })
    if $has_wrapper and (not ($from_notes | is-empty)) {
      $violations = ($violations | append $from_notes)
    } else {
      let combined = ($from_msg | append $from_notes)
      if not ($combined | is-empty) {
        $violations = ($violations | append $combined)
      } else if $sev == "Error" {
        # No coded line: keep every message line and every note so the cause
        # (for example "cannot merge unequal arrays") stays actionable.
        let parts = ([$msg] | append $notes | where {|x| ($x | str trim) != "" })
        let detail = if ($parts | is-empty) {
          "nickel export failed without diagnostics"
        } else {
          $parts | str join "\n" | str trim
        }
        $violations = ($violations | append { code: "NICKEL_ERROR", detail: $detail })
      }
    }
  }
  if ($violations | is-empty) {
    $violations = [{ code: "NICKEL_ERROR", detail: "nickel export failed without diagnostics" }]
  }
  {
    violations: ($violations | uniq)
    blame: $parsed
  }
}

def is-valid-ident [s: string] {
  ($s != "") and ($s =~ '^[a-z0-9][a-z0-9-]{0,63}$')
}

def is-safe-relpath [rel: string] {
  if $rel == "" or ($rel | str starts-with "/") or ($rel | str ends-with "/") or ($rel | str starts-with "~") {
    return false
  }
  let parts = ($rel | split row "/")
  not ($parts | any {|p| ($p == "") or ($p == ".") or ($p == "..") })
}

# Find a hub at the flat or uppercase path for a product/slug, then read its
# frontmatter status. Returns [] when no hub exists.
def hub_status [root: string, product: string, slug: string] {
  let lower = $"($root)/foundry/($product)/($slug).md"
  let upper = $"($root)/foundry/($product)/($slug | str uppercase).md"
  let hub = if ($lower | path exists) { $lower } else if ($upper | path exists) { $upper } else { "" }
  if $hub == "" { return [] }
  let text = (try { open --raw $hub } catch { "" })
  if not ($text | str starts-with "---") { return ["missing"] }
  let lines = ($text | lines)
  let end = ($lines | enumerate | skip 1 | where {|x| ($x.item | str trim) == "---"} | get index? | first | default null)
  if $end == null { return ["missing"] }
  let fm = ($lines | skip 1 | take ($end - 1))
  let row = ($fm | where {|l| $l =~ '^status:\s*' } | first | default "")
  if $row == "" { return ["missing"] }
  let val = ($row | str replace -r '^status:\s*' "" | str trim | str trim -c '"' | str trim -c "'")
  [ $val ]
}

def inside_schema [path: string, schema_root: string] {
  let p = ($path | path expand | str trim -r -c "/")
  let s = ($schema_root | path expand | str trim -r -c "/")
  $p == $s or ($p | str starts-with $"($s)/")
}

def main [
  ncl_file: string
  --vault: string = ""
  --facts: string = ""
  --nickel: string = ""
  --force
  --dry-run
] {
  let t0 = (date now)
  if not ($ncl_file | path exists) {
    fail "materialize" 2 $"nickel file not found: ($ncl_file)"
    return
  }
  if (not $dry_run) and $vault == "" {
    fail "materialize" 2 "missing --vault <dir> (or pass --dry-run)"
    return
  }
  let root = if $vault == "" { "" } else { $vault | path expand | str trim -r -c "/" }
  let nk = (resolve-nickel $nickel)

  # 1. Evaluate Nickel pack -> JSON. With --facts, a temporary wrapper
  # outside the vault re-renders the exported law record with live facts.
  let tn0 = (date now)
  mut wrapper_dir = ""
  let eval_file = if $facts != "" {
    if not ($facts | path exists) {
      fail "materialize" 2 $"facts file not found: ($facts)"
      return
    }
    let fdoc = (try { open --raw $facts | from json } catch { null })
    if $fdoc == null {
      fail "materialize" 2 $"invalid JSON in facts file: ($facts)"
      return
    }
    let d = (^mktemp -d | complete)
    if $d.exit_code != 0 {
      fail "materialize" 2 $"failed to create facts wrapper dir: ($d.stderr | str trim)"
      return
    }
    $wrapper_dir = ($d.stdout | str trim)
    let w = $"($wrapper_dir)/with-facts.ncl"
    let base = ($ncl_file | path expand | to json)
    let facts_path = ($facts | path expand | to json)
    ("let base = import " + $base + " in\n" + "{ out = base.with_facts (import " + $facts_path + ") }\n") | save -f $w
    $w
  } else {
    $ncl_file
  }
  let res = (try {
    ^$nk export --format json --error-format json $eval_file | complete
  } catch {|e|
    let why = ($e.msg? | default "failed to invoke nickel")
    { exit_code: 127, stdout: "", stderr: $"nickel: ($why)" }
  })
  if $wrapper_dir != "" { rm -rf $wrapper_dir }
  let nickel_ms = (((date now) - $tn0) / 1ms | math round -p 1)

  if $res.exit_code == 127 {
    # A missing binary is the envelope, never a crash or a non-zero exit.
    let why = ($res.stderr | str trim)
    fail "materialize" 127 (if $why == "" { "failed to invoke nickel" } else { $why })
    return
  }

  if $res.exit_code != 0 {
    let raw_err = if ($res.stderr | str trim) != "" { $res.stderr } else { $res.stdout }
    let diag = (parse-nickel-diagnostics $raw_err)
    let total_ms = (((date now) - $t0) / 1ms | math round -p 1)
    emit true "materialize" 2 {
      passed: false
      stage: "nickel"
      nickel_ms: $nickel_ms
      total_ms: $total_ms
      violation_count: ($diag.violations | length)
      violations: $diag.violations
      blame: $diag.blame
    } null
    return
  }

  let exported = (try { $res.stdout | from json } catch { null })
  let exp = if $facts != "" { $exported | get -o out } else { $exported }
  if $exp == null or (($exp | describe) | str starts-with "record") == false {
    fail "materialize" 2 "nickel export did not produce a JSON object"
    return
  }
  let iface = ($exp | get -o interface | default "")
  if $iface != "proto-ncl-pack/v0" {
    fail "materialize" 2 $"unsupported interface '($iface)' \(expected 'proto-ncl-pack/v0'\)"
    return
  }

  let product = ($exp | get -o product | default "")
  let slug = ($exp | get -o slug | default "")
  let roles = ($exp | get -o roles | default [])
  let owners = ($exp | get -o owners | default [])
  let files = ($exp | get -o files | default [])
  let fence = ($exp | get -o fence | default {})

  # Defence in depth: never trust upstream law.ncl alone for product/slug/role idents.
  mut ident_violations = []
  if not (is-valid-ident $product) {
    $ident_violations = ($ident_violations | append { code: "IDENT_INVALID", detail: $"invalid product identifier: '($product)'" })
  }
  if not (is-valid-ident $slug) {
    $ident_violations = ($ident_violations | append { code: "IDENT_INVALID", detail: $"invalid slug identifier: '($slug)'" })
  }
  for r in $roles {
    if not (is-valid-ident ($r | into string)) {
      $ident_violations = ($ident_violations | append { code: "IDENT_INVALID", detail: $"invalid role identifier: '($r)'" })
    }
  }
  if not ($ident_violations | is-empty) {
    let total_ms = (((date now) - $t0) / 1ms | math round -p 1)
    emit true "materialize" 2 {
      passed: false
      stage: "ident"
      nickel_ms: $nickel_ms
      total_ms: $total_ms
      violation_count: ($ident_violations | length)
      violations: $ident_violations
      blame: null
    } null
    return
  }

  # B2: an input pack inside schema/<product>/<slug> would be destroyed when
  # --force replaces that directory. Refuse before any write.
  if (not $dry_run) and $force and $root != "" {
    let input = if ($ncl_file | str starts-with "/") { $ncl_file } else { (pwd) + "/" + $ncl_file }
    let schema_root = $"($root)/foundry/($product)/schema/($slug)"
    if (inside_schema $input $schema_root) {
      let total_ms = (((date now) - $t0) / 1ms | math round -p 1)
      emit true "materialize" 2 {
        passed: false
        stage: "path"
        nickel_ms: $nickel_ms
        total_ms: $total_ms
        violation_count: 1
        violations: [{ code: "PATH_UNSAFE", detail: $"input pack ('($ncl_file)') resolves inside ($schema_root)" }]
        blame: null
      } null
      return
    }
  }

  # B1: never overwrite a pack once the hub leaves awaiting-approval.
  if (not $dry_run) and $force and $root != "" {
    let statuses = (hub_status $root $product $slug)
    if not ($statuses | is-empty) and (($statuses | first) != "awaiting-approval") {
      let total_ms = (((date now) - $t0) / 1ms | math round -p 1)
      emit true "materialize" 2 {
        passed: false
        stage: "hub"
        nickel_ms: $nickel_ms
        total_ms: $total_ms
        violation_count: 1
        violations: [{ code: "PACK_LOCKED", detail: $"existing hub status is '($statuses | first)'; only awaiting-approval may be regenerated" }]
        blame: null
      } null
      return
    }
  }

  # 2. Live seat verification against facts.json (G_OWNER_UNKNOWN)
  mut roles_verified = []
  if $facts != "" {
    if not ($facts | path exists) {
      fail "materialize" 2 $"facts file not found: ($facts)"
      return
    }
    let fdoc = (try { open --raw $facts | from json } catch { null })
    if $fdoc == null {
      fail "materialize" 2 $"invalid JSON in facts file: ($facts)"
      return
    }
    let live_roles = ($fdoc | get -o seats | default [] | each {|s| $s | get -o role } | where {|r| $r != null } | uniq)
    mut seat_violations = []
    mut checked_roles = []
    for r in $roles {
      if $r not-in $live_roles {
        $seat_violations = ($seat_violations | append { code: "G_OWNER_UNKNOWN", detail: $"declared role '($r)' is not in facts.seats" })
      } else {
        $checked_roles = ($checked_roles | append $r)
      }
    }
    for o in $owners {
      let orole = ($o | get -o role | default "")
      let ostep = ($o | get -o step | default "")
      if $orole not-in $live_roles {
        $seat_violations = ($seat_violations | append { code: "G_OWNER_UNKNOWN", detail: $"step '($ostep)' owner '($orole)' is not in facts.seats" })
      } else {
        $checked_roles = ($checked_roles | append $orole)
      }
    }
    $seat_violations = ($seat_violations | uniq)
    if not ($seat_violations | is-empty) {
      let total_ms = (((date now) - $t0) / 1ms | math round -p 1)
      emit true "materialize" 2 {
        passed: false
        stage: "facts"
        nickel_ms: $nickel_ms
        total_ms: $total_ms
        violation_count: ($seat_violations | length)
        violations: $seat_violations
        blame: null
      } null
      return
    }
    $roles_verified = ($checked_roles | uniq)
  }

  # 3. Path safety + pre-write collision check (zero writes on failure)
  let root = if $vault == "" { "" } else { $vault | path expand | str trim -r -c "/" }
  mut pre_violations = []
  mut to_write = []
  mut to_skip = []

  let expected_prefix = $"foundry/($product)/"
  for f in $files {
    let rel = ($f | get -o path | default "")
    let create_only = ($f | get -o create_only | default false)
    if (not (is-safe-relpath $rel)) or (not ($rel | str starts-with $expected_prefix)) {
      $pre_violations = ($pre_violations | append { code: "PATH_UNSAFE", detail: $"unsafe relative path in files[]: '($rel)'" })
      continue
    }
    if $root != "" {
      let dst = $"($root)/($rel)"
      let exists = ($dst | path exists)
      if $exists and $create_only {
        $to_skip = ($to_skip | append $rel)
      } else if $exists and (not $force) {
        $pre_violations = ($pre_violations | append { code: "STAGE_COLLISION", detail: $"target already exists: ($rel)" })
      } else {
        $to_write = ($to_write | append $f)
      }
    } else {
      $to_write = ($to_write | append $f)
    }
  }

  if not ($pre_violations | is-empty) {
    let total_ms = (((date now) - $t0) / 1ms | math round -p 1)
    emit true "materialize" 2 {
      passed: false
      stage: "collision"
      nickel_ms: $nickel_ms
      total_ms: $total_ms
      violation_count: ($pre_violations | length)
      violations: $pre_violations
      blame: null
    } null
    return
  }

  # 4. Atomic staging in sibling mktemp -d + per-file & schema/<slug>/ rename
  mut written_paths = []
  if not $dry_run {
    mkdir $root
    let stage_parent = ($root | path dirname)
    let mk = (^mktemp -d $"($stage_parent)/.proto-stage.XXXXXX" | complete)
    if $mk.exit_code != 0 {
      fail "materialize" 2 $"failed to create staging dir: ($mk.stderr | str trim)"
      return
    }
    let stage = ($mk.stdout | str trim)
    try {
    let schema_prefix = $"foundry/($product)/schema/($slug)/"
    mut schema_files = []
    mut other_files = []

    for f in $to_write {
      let rel = $f.path
      let st_dst = $"($stage)/($rel)"
      let st_parent = ($st_dst | path dirname)
      mkdir $st_parent
      ($f.content | into string) | save -f $st_dst
      if ($rel | str starts-with $schema_prefix) {
        $schema_files = ($schema_files | append $rel)
      } else {
        $other_files = ($other_files | append $rel)
      }
    }

    # Commit schema/<slug>/ as one directory rename when present
    if not ($schema_files | is-empty) {
      let st_schema_dir = $"($stage)/foundry/($product)/schema/($slug)"
      let dst_schema_parent = $"($root)/foundry/($product)/schema"
      let dst_schema_dir = $"($dst_schema_parent)/($slug)"
      # Defence in depth: assert exact rm -rf target before any destructive delete.
      if (not (is-valid-ident $product)) or (not (is-valid-ident $slug)) or ($dst_schema_dir != $"($root)/foundry/($product)/schema/($slug)") or (($dst_schema_dir | path dirname) != $dst_schema_parent) or (($dst_schema_dir | path basename) != $slug) {
        rm -rf $stage
        emit true "materialize" 2 {
          passed: false
          stage: "schema"
          violation_count: 1
          violations: [{ code: "PATH_UNSAFE", detail: $"refusing unsafe schema target: ($dst_schema_dir)" }]
          blame: null
        } null
        return
      }
      mkdir $dst_schema_parent
      if $force and ($dst_schema_dir | path exists) {
        rm -rf $dst_schema_dir
      }
      ^mv $st_schema_dir $dst_schema_dir
      $written_paths = ($written_paths | append $schema_files)
    }

    # Commit remaining files via same-filesystem atomic rename
    for rel in $other_files {
      let st_src = $"($stage)/($rel)"
      let dst = $"($root)/($rel)"
      mkdir ($dst | path dirname)
      ^mv -f $st_src $dst
      $written_paths = ($written_paths | append $rel)
    }

    } catch {|e|
      rm -rf $stage
      fail "materialize" 2 $"staging failed: ($e.msg)"
      return
    }
    rm -rf $stage
  } else {
    $written_paths = ($to_write | each {|f| $f.path })
  }

  let total_ms = (((date now) - $t0) / 1ms | math round -p 1)
  emit true "materialize" 0 {
    passed: true
    interface: $iface
    product: $product
    slug: $slug
    dry_run: $dry_run
    fence: $fence
    nickel_ms: $nickel_ms
    total_ms: $total_ms
    files_written: $written_paths
    files_skipped: $to_skip
    roles_verified: $roles_verified
  } null
}
