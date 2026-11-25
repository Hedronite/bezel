# law.ncl: public goal-pack law (interface proto-ncl-pack/v0).
# Generic contracts and renderer. Holds no host, seat, person or vault names.
# Compose: law & org overlay & pack intent, then export `.out`.
let LAW_FENCE_MAX = 3800 in
let LAW_STANDING = ["S1", "S2", "S3", "S4"] in
let LAW_FORBIDDEN_PHRASES = ["do not run /goal", "says GO"] in
let LAW_FORBIDDEN = ["pushing main", "force-push", "merging on red CI"] in

# ---------- helpers ----------
let join = std.string.join in
let len = std.string.length in
let starts_with = fun prefix s =>
  len s >= len prefix && std.string.substring 0 (len prefix) s == prefix in
let strip_slash = fun p =>
  if std.string.is_match "/$" p then std.string.substring 0 (len p - 1) p else p in
let paths_collide = fun a0 b0 =>
  let a = strip_slash a0 in
  let b = strip_slash b0 in
  a == b || starts_with (a ++ "/") b || starts_with (b ++ "/") a in
let num = std.string.from_number in
let uniq = fun arr =>
  std.array.fold_left (fun acc x => if std.array.elem x acc then acc else acc @ [x]) [] arr in
let present = std.array.filter (fun x => x != null) in

# ---------- field contracts (survive every merge, any priority) ----------
let FenceCap = std.contract.custom (fun _label v =>
  if std.typeof v != 'Number then 'Error { message = "FENCE_CAP_WEAKENED: fence_max must be a number" }
  else if v <= LAW_FENCE_MAX then 'Ok v
  else 'Error { message = "FENCE_CAP_WEAKENED: fence_max %{num v} exceeds the law cap %{num LAW_FENCE_MAX}" }) in

let Standing = std.contract.custom (fun _label v =>
  let ids = std.array.map (fun r => r.id) v in
  let missing = std.array.filter (fun s => !(std.array.elem s ids)) LAW_STANDING in
  if missing == [] then 'Ok v
  else 'Error { message = "STANDING_WEAKENED: standing rows missing %{join ", " missing}" }) in

let PhraseFloor = std.contract.custom (fun _label v =>
  let missing = std.array.filter (fun s => !(std.array.elem s v)) LAW_FORBIDDEN_PHRASES in
  if missing == [] then 'Ok v
  else 'Error { message = "PHRASES_WEAKENED: forbidden_phrases dropped %{join ", " missing}" }) in

# Layered additive lists never lose an upper layer. The upper layer declares
# its items once and binds this contract to the same field it sets; field
# contracts survive every later merge, including `| priority 10` and `| force`,
# so a downstream layer can add items but can never silently drop one.
# Overlay: let rules = ["..."] in { org_forbidden | law.list_floor rules = rules }
let ListFloor = fun required =>
  std.contract.custom (fun _label v =>
    if std.typeof v != 'Array then
      'Error { message = "LAYER_WEAKENED: expected an array of strings" }
    else
      let vals = std.array.map (fun x => std.to_string x) v in
      let missing = std.array.filter (fun s => !(std.array.elem s vals)) required in
      if missing == [] then 'Ok v
      else 'Error { message = "LAYER_WEAKENED: final list dropped upper-layer entries: %{join ", " missing}" }) in

# Names that become path segments: product, slug, role. An empty or odd name
# would turn schema/<slug>/ into schema/ and widen every later rename or delete.
let Ident = std.contract.custom (fun _label v =>
  if std.typeof v == 'String && std.string.is_match "^[a-z0-9][a-z0-9-]{0,63}$" v then 'Ok v
  else 'Error { message = "IDENT_INVALID: '%{std.to_string v}' is not a lower-case identifier ([a-z0-9][a-z0-9-]*)" }) in

let Step = {
  id | String,
  title | String,
  role | String | Ident,
  paths | Array String | default = [],
  pass | String,
  fail | String,
  depends_on | Array String | default = [],
} in

# ---------- whole-pack contract: reports every violation at once ----------
let PackValid = std.contract.custom (fun _label v =>
  let fence = v.fence_text in
  let lower = std.string.lowercase fence in
  let ids = std.array.map (fun s => s.id) v.steps in
  let dup_ids = uniq (std.array.filter
    (fun i => std.array.length (std.array.filter (fun j => j == i) ids) > 1) ids) in
  let bad_ids = uniq (std.array.filter
    (fun i => !(std.string.is_match "^G[0-9]+[a-z]?$" i)) ids) in
  let undeclared = uniq (std.array.filter (fun r => !(std.array.elem r v.roles))
    (std.array.map (fun s => s.role) v.steps)) in
  let claims = std.array.flat_map
    (fun s => std.array.map (fun p => { step = s.id, role = s.role, path = p }) s.paths) v.steps in
  let n = std.array.length claims in
  let overlaps = std.array.flat_map (fun i =>
      std.array.flat_map (fun j =>
          let a = std.array.at i claims in
          let b = std.array.at j claims in
          if j > i && a.role != b.role && paths_collide a.path b.path
          then ["OWNER_OVERLAP: %{a.step} (%{a.role}) %{a.path} collides with %{b.step} (%{b.role}) %{b.path}"]
          else [])
        (std.array.range 0 n))
    (std.array.range 0 n) in
  let seat_texts = fun s => present [
    (if std.record.has_field "role" s then s.role else null),
    (if std.record.has_field "pane_id" s then s.pane_id else null),
    (if std.record.has_field "tab_id" s then s.tab_id else null),
    (if std.record.has_field "harness" s then s.harness else null),
    (if std.record.has_field "model" s then s.model else null),
    (if std.record.has_field "seat_label" s then s.seat_label else null),
    (if std.record.has_field "workspace" s then s.workspace else null),
    (if std.record.has_field "captured" s then s.captured else null),
  ] in
  let bad_text = fun x => std.typeof x == 'String && (
    std.string.contains "\n" x || std.string.contains "\r" x ||
    std.string.contains "```" x || std.string.contains "[[" x) in
  let raw_facts = [v.facts.occupancy, v.facts.workspace, v.facts.captured]
    @ std.array.flat_map seat_texts v.facts.seats in
  let bad_facts = std.array.filter bad_text raw_facts in
  let vs = present [
      (if len fence > v.fence_max
       then "FENCE_OVER_CAP: goal fence is %{num (len fence)} chars (cap %{num v.fence_max})" else null),
      (if std.string.is_match "^[\\x00-\\x7F]*$" fence then null
       else "FENCE_NON_ASCII: goal fence holds a non-ASCII character"),
      (if std.array.length bad_facts == 0 then null
       else "FACTS_INVALID: seat or recon text contains a newline, triple backtick, or wikilink opener"),
    ]
    @ std.array.flat_map
        (fun p => if std.string.contains (std.string.lowercase p) lower
                  then ["FENCE_FORBIDDEN: goal fence contains '%{p}'"] else [])
        v.forbidden_phrases
    @ std.array.map (fun i => "STEP_ID_DUP: step id %{i} appears more than once") dup_ids
    @ std.array.map (fun i => "STEP_ID_INVALID: step id '%{i}' does not match ^G[0-9]+[a-z]?$") bad_ids
    @ std.array.map (fun r => "OWNER_ROLE_UNDECLARED: role %{r} owns a step but is not in roles") undeclared
    @ overlaps in
  if vs == [] then 'Ok v
  else 'Error {
    message = "PACK_INVALID: %{num (std.array.length vs)} violation(s)",
    notes = vs,
  }) in

{
  # ---------- helper exports (public, for overlays and pack intents) ----------
  # Bind to any additive list an upper layer sets:
  #   let rules = ["..."] in { org_forbidden | law.list_floor rules = rules }
  list_floor = ListFloor,

  # ---------- law knobs: caps tighten only, rows and phrases only grow ----------
  fence_max | FenceCap | default = LAW_FENCE_MAX,
  forbidden_phrases | Array String | PhraseFloor | default = LAW_FORBIDDEN_PHRASES,
  standing | Array { id | String, task | String, pass | String, fail | String } | Standing
    | default = [
      { id = "S1", task = "QA grades every lane that wrote, same turn as READY",
        pass = "QA evidence filed same turn as READY", fail = "QA late or self-grading" },
      { id = "S2", task = "touched Markdown gets YAML frontmatter and resolving wikilinks inside the step",
        pass = "All touched Markdown has valid YAML and resolving wikilinks", fail = "Missing YAML or dangling link" },
      { id = "S3", task = "every finished todo ticked or blocker named",
        pass = "All completed todos ticked or blocker named", fail = "Unticked finished work" },
      { id = "S4", task = "one ready PR per step; merged only after green CI and QA clear",
        pass = "Merged only after green CI and exact-head clear", fail = "Merged on red CI or unverified head" },
    ],
  # Additive lists are per-layer: the law floor, the org overlay, and the pack
  # intent each own a field the law concatenates. `forbidden` is floor-guarded
  # by the law; an overlay guards its own list with `list_floor` (see below).
  forbidden | Array String | ListFloor LAW_FORBIDDEN | default = LAW_FORBIDDEN,
  org_forbidden | Array String | default = [],
  pack_forbidden | Array String | default = [],

  # ---------- product knobs (default: any overlay may set them) ----------
  hub_case | [| 'lower, 'upper |] | default = 'lower,
  doc_owner | String | default = "pm",
  build_rule | String | default = "build on the build host named in the SPEC; never on the cockpit host",
  merge_rule | String | default = "writers never merge; merge only after green CI and exact-head clear",
  date | String | default = "1970-01-01",
  layout | [| 'flat, 'nested |] | default = 'flat,
  status | [| 'awaiting-approval, 'pinged, 'execution-authorized, 'done |] | default = 'awaiting-approval,

  # ---------- pack intent (no defaults: the pack must say these) ----------
  product | String | Ident,
  slug | String | Ident,
  title | String,
  roles | Array Ident,
  steps | Array Step,
  review_body | String,
  spec_body | String,
  # Live seat facts, supplied by `materialize.nu --facts` through a temporary
  # wrapper. The default keeps intents without facts byte-identical.
  facts | {
    occupancy | String | default = "unverified",
    workspace | String | default = "",
    captured | String | default = "",
    seats | Array Dyn | default = [],
  } | default = { occupancy = "unverified", workspace = "", captured = "", seats = [] },

  # ---------- derived ----------
  writers = uniq (std.array.map (fun s => s.role) steps),
  facts_norm = fun f => {
    occupancy = if std.record.has_field "occupancy" f then f.occupancy else "unverified",
    workspace = if std.record.has_field "workspace" f then f.workspace else "",
    captured = if std.record.has_field "captured" f then f.captured else "",
    seats = if std.record.has_field "seats" f then f.seats else [],
  },
  active_seats = fun f =>
    let nf = facts_norm f in
    std.array.filter (fun s =>
      std.record.has_field "role" s && std.array.elem s.role roles) nf.seats,
  seat_writes = fun s =>
    if std.record.has_field "writes" s then s.writes else std.array.elem s.role writers,
  seat_idle = fun s =>
    if std.record.has_field "idle_is_fail" s then s.idle_is_fail else true,
  forbidden_all = uniq (forbidden @ org_forbidden @ pack_forbidden),
  p_rel = "foundry/%{product}",
  hub_stem = if hub_case == 'upper then std.string.uppercase slug else slug,
  doc_rel = fun dir upper =>
    let stem = "%{upper}-%{slug}" in
    if layout == 'nested then "%{dir}/%{stem}" else "%{stem}",
  link_rel = fun dir upper => "%{p_rel}/%{doc_rel dir upper}",
  file_rel = fun dir upper => "%{doc_rel dir upper}.md",
  wl = {
    hub = "%{p_rel}/%{hub_stem}",
    review = link_rel "reviews" "REVIEW",
    spec = link_rel "specs" "SPEC",
    checklist = link_rel "checklists" "CHECKLIST",
    rubric = link_rel "rubrics" "RUBRIC",
    goal = link_rel "goals" "GOAL",
    status = "%{p_rel}/STATUS",
    goal_schema = "%{p_rel}/schema/%{slug}/goal.schema.json",
    goal_instance = "%{p_rel}/schema/%{slug}/goal.instance.json",
  },
  link = fun t => "[[%{t}]]",
  seat_desc = fun s =>
    let pane = if std.record.has_field "pane_id" s then s.pane_id else "" in
    let tab = if std.record.has_field "tab_id" s then s.tab_id else "" in
    let harness = if std.record.has_field "harness" s then s.harness else "" in
    let model = if std.record.has_field "model" s then s.model else "" in
    let writes = seat_writes s in
    let idle = seat_idle s in
    let loc = if pane != "" then pane else tab in
    let loc_s = if loc == "" then "" else " %{loc}" in
    let harness_s = if harness == "" then "" else " %{harness}" in
    let model_s = if model == "" then "" else " %{model}" in
    "%{s.role}%{loc_s}%{harness_s}%{model_s} writes=%{if writes then "yes" else "no"} idle=%{if idle then "fail" else "ok"}",
  seats_text_for = fun fx =>
    let active = active_seats fx in
    if std.array.length active == 0
    then "%{join ", " roles}; idle is fail"
    else join "; " (std.array.map seat_desc active),
  seats_text = seats_text_for facts,

  fence_text_for = fun fx => join "\n" [
    "You are PM looping %{product} %{slug}. You write no product code.",
    "",
    "Read after compact, in order: the product repo AGENTS.md, %{link wl.spec}, %{link wl.goal}, %{link wl.checklist}, %{link wl.rubric}, %{link wl.review}, %{link wl.status}.",
    "",
    "DONE only when %{link wl.checklist} %{join ", " (std.array.map (fun s => s.id) steps)} and %{join ", " (std.array.map (fun r => r.id) standing)} pass under %{link wl.rubric}. Paging a seat is not done.",
    "",
    "SQUAD: %{join "; " (std.array.map (fun s => "%{s.id} %{s.role}: %{s.title}") steps)}.",
    "SEATS: %{seats_text_for fx}.",
    "HOSTS: %{build_rule}.",
    "MERGE: %{merge_rule}.",
    "FORBIDDEN: %{join "; " forbidden_all}.",
  ],

  fence_text = fence_text_for facts,

  fm = fun r =>
    let tags = join "\n" (std.array.map (fun t => "  - %{t}") r.tags) in
    let refs = if r.refs == [] then ""
      else "cross_refs:\n" ++ join "\n" (std.array.map (fun t => "  - \"%{link t}\"") r.refs) ++ "\n" in
    let extra = join "" (std.array.map (fun kv => "%{kv.k}: \"%{kv.v}\"\n") r.extra) in
    "---\ntitle: \"%{r.title}\"\ntype: %{r.type}\nstatus: %{r.status}\ndomain: foundry\ncreated: \"%{date}\"\nupdated: \"%{date}\"\nowner: %{doc_owner}\n%{extra}%{refs}tags:\n%{tags}\n---\n\n",

  mkdoc = fun kind body xrefs => {
    title = "%{kind} - %{product} %{slug}",
    content = fm {
        title = "%{kind} - %{product} %{slug}", type = "foundry-doc", status = "spec-locked",
        extra = [{ k = "parent", v = link wl.hub }, { k = "schema", v = link wl.goal_schema }],
        refs = xrefs, tags = [product, slug, std.string.lowercase kind],
      } ++ "# %{kind}-%{slug}\n\n" ++ body ++ "\n",
  },

  checklist_body = join "\n" (
    std.array.flat_map (fun s => ["## %{s.id}", "- [ ] %{s.id} %{s.title} (%{s.role})", ""]) steps
    @ ["## Standing (every step)"]
    @ std.array.map (fun r => "- [ ] %{r.id} %{r.task}") standing),

  rubric_body = join "\n" (
    ["| ID | Pass when | Fail when |", "|---|---|---|"]
    @ std.array.map (fun s => "| %{s.id} | %{s.pass} | %{s.fail} |") steps
    @ std.array.map (fun r => "| %{r.id} | %{r.pass} | %{r.fail} |") standing),

  wiki_vals = {
      hub = link wl.hub, review = link wl.review, spec = link wl.spec, checklist = link wl.checklist,
      rubric = link wl.rubric, goal = link wl.goal, status = link wl.status,
    }
    & std.record.from_array (std.array.map
        (fun w => { field = "brief_%{w}", value = link "%{p_rel}/briefs/%{slug}-%{w}" }) writers),

  schema_id = fun name => "https://hedronite.local/%{product}/schema/%{slug}/%{name}",

  json = fun v => std.serialize 'Json v ++ "\n",

  # Record fields are recursive: `x = x` inside a record literal refers to itself,
  # so outer values are bound with `let` before they are copied into a literal.
  view_for = fun fx =>
    let nf = facts_norm fx in
    let ft = fence_text_for nf in let fm = fence_max in let fp = forbidden_phrases in
    let st = steps in let ro = roles in
    { fence_text = ft, fence_max = fm, forbidden_phrases = fp, steps = st, roles = ro, facts = nf },
  view = view_for facts,

  # ---------- the export (interface proto-ncl-pack/v0) ----------
  # `render_with_facts` lets materialize.nu re-render a pack after merging a
  # facts record, without changing the user's pack.ncl expression.
  render_with_facts = fun fx =>
    let nf = facts_norm fx in
    let checked = view_for nf | PackValid in
    let f = fun rel body => { path = "%{p_rel}/%{rel}", content = body, create_only = false } in
    let the_product = product in
    let the_slug = slug in
    let the_status = std.string.from_enum status in
    let the_standing = standing in
    {
      interface = "proto-ncl-pack/v0",
      product = the_product,
      slug = the_slug,
      fence = { chars = len checked.fence_text, cap = checked.fence_max },
      roles = checked.roles,
      owners = std.array.map (fun s => { step = s.id, role = s.role, paths = s.paths }) checked.steps,
      files = [
        { path = "%{p_rel}/STATUS.md", create_only = true,
          content = fm { title = "STATUS - %{product}", type = "living", status = "in-progress",
                         extra = [], refs = [], tags = [product, "status"] }
                    ++ "# STATUS - %{product}\n\nPack `%{slug}` filed %{date} (`%{the_status}`).\n" },
        f "%{hub_stem}.md" (
          fm { title = "%{product} %{slug} pack hub", type = "foundry-doc", status = the_status,
               extra = [{ k = "parent", v = link wl.status }, { k = "schema", v = link wl.goal_schema },
                        { k = "instance", v = link wl.goal_instance }],
               refs = [wl.review, wl.spec, wl.checklist, wl.rubric, wl.goal, wl.status],
               tags = [product, slug, "hub"] }
          ++ "# %{title}\n\n"
          ++ join "\n" [ "- Review: %{link wl.review}", "- Spec: %{link wl.spec}", "- Checklist: %{link wl.checklist}",
                         "- Rubric: %{link wl.rubric}", "- Goal: %{link wl.goal}", "- Status: %{link wl.status}" ]
          ++ "\n"),
        f (file_rel "reviews" "REVIEW") (mkdoc "REVIEW" review_body [wl.spec, wl.checklist, wl.rubric, wl.goal]).content,
        f (file_rel "specs" "SPEC") (mkdoc "SPEC" (spec_body ++ "\n\n## File owners\n\n"
            ++ join "\n" (std.array.map (fun s => "- %{s.id} (%{s.role}): %{if s.paths == [] then "no files" else join ", " (std.array.map (fun p => "`%{p}`") s.paths)}") steps))
            [wl.review, wl.checklist, wl.rubric, wl.goal]).content,
        f (file_rel "checklists" "CHECKLIST") (mkdoc "CHECKLIST" checklist_body [wl.spec, wl.rubric, wl.goal, wl.review]).content,
        f (file_rel "rubrics" "RUBRIC") (mkdoc "RUBRIC" rubric_body [wl.checklist, wl.spec, wl.goal, wl.review]).content,
        f (file_rel "goals" "GOAL") (mkdoc "GOAL"
            ("## Authorized goal text\n\n```\n%{checked.fence_text}\n```\n\nGoal text: %{num (len checked.fence_text)} characters (cap %{num checked.fence_max}).")
            [wl.spec, wl.checklist, wl.rubric, wl.review, wl.hub, wl.status]).content,
        f "schema/%{slug}/wiki.schema.json" (json {
          "$schema" = "https://json-schema.org/draft/2020-12/schema",
          "$id" = schema_id "wiki.schema.json",
          title = "Wiki-link refs for %{product} %{slug}",
          type = "object",
          additionalProperties = false,
          required = std.record.fields wiki_vals,
          properties = std.record.map (fun _k v => { const = v }) wiki_vals,
        }),
        f "schema/%{slug}/goal.schema.json" (json {
          "$schema" = "https://json-schema.org/draft/2020-12/schema",
          "$id" = schema_id "goal.schema.json",
          title = "Goal pack schema for %{product} %{slug}",
          type = "object",
          required = ["id", "version", "status", "wiki", "squad", "sequence", "standing", "forbidden"],
          properties = {
            id = { type = "string" }, version = { type = "string" },
            status = { enum = ["awaiting-approval", "pinged", "execution-authorized", "done"] },
            wiki = { "$ref" = "./wiki.schema.json" }, squad = { type = "object" },
            sequence = { type = "array" },
            standing = { type = "array", items = { type = "string" } },
            forbidden = { type = "array", items = { type = "string" } },
          },
        }),
        f "schema/%{slug}/goal.instance.json" (json {
          "$schema" = "./goal.schema.json",
          id = "%{std.string.uppercase product}-%{std.string.uppercase slug}-GOAL",
          version = "1.0.0",
          status = the_status,
          wiki = wiki_vals,
          squad = {
            recon = nf.occupancy, workspace = nf.workspace, captured = nf.captured,
            idle_is_fail = true,
            seats = if std.array.length (active_seats nf) == 0 then
              std.array.map (fun r => { role = r, writes = std.array.elem r writers, idle_is_fail = true }) roles
            else
              std.array.map (fun s => {
                role = s.role,
                pane_id = if std.record.has_field "pane_id" s then s.pane_id else "",
                tab_id = if std.record.has_field "tab_id" s then s.tab_id else "",
                harness = if std.record.has_field "harness" s then s.harness else "",
                model = if std.record.has_field "model" s then s.model else "",
                writes = seat_writes s,
                idle_is_fail = seat_idle s,
              }) (active_seats nf),
          },
          sequence = std.array.map (fun s => { step = s.id, ids = [s.id], owner = s.role, depends_on = s.depends_on }) steps,
          standing = std.array.map (fun r => r.id) the_standing,
          forbidden = forbidden_all,
        }),
        f "schema/%{slug}/README.md" (
          fm { title = "Schema README - %{product} %{slug}", type = "foundry-doc", status = "spec-locked",
               extra = [{ k = "parent", v = link wl.hub }], refs = [wl.goal, wl.spec],
               tags = [product, slug, "schema"] }
          ++ "# Schema graph - %{product} %{slug}\n\n- `goal.instance.json` -> `goal.schema.json` -> `wiki.schema.json`\n"),
      ]
      @ std.array.map (fun w =>
          f "briefs/%{slug}-%{w}.md" (
            fm { title = "Brief - %{product} %{slug} (%{w})", type = "foundry-doc", status = "spec-locked",
                 extra = [{ k = "parent", v = link wl.hub }, { k = "schema", v = link wl.goal_schema }],
                 refs = [wl.spec, wl.checklist, wl.rubric, wl.goal], tags = [product, slug, "brief", w] }
            ++ "# Brief - %{w} (%{product}/%{slug})\n\nDark until PM pages. Read %{link wl.spec}, %{link wl.checklist}, and %{link wl.rubric}.\n"))
        writers,
    },
  out =
    let rwf = render_with_facts in
    let r = rwf facts in
    r & {
      render_with_facts | not_exported = rwf,
      with_facts | not_exported = fun fx => rwf fx,
    },
}
