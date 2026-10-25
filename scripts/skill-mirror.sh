# Shared regular-file skill tree copy. Source this file; do not execute it.
# A destination matches when every path is a real file or directory and the
# bytes match. Symlinks are drift, including when the target bytes match.

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  printf 'skill-mirror.sh is sourced by the skill scripts\n' >&2
  exit 2
fi

skill_fail() {
  printf 'skill-mirror: %s\n' "$*" >&2
  exit 1
}

# Immediate child directories of a skills root. Each one must contain SKILL.md.
skill_dir_names() {
  local root=$1
  local path name
  local -a names=()
  if [[ -L $root || ! -d $root ]]; then
    skill_fail "skills root is not a real directory: $root"
  fi
  shopt -s nullglob
  for path in "$root"/*/; do
    name=$(basename "${path%/}")
    if [[ -L $root/$name ]]; then
      skill_fail "skill directory is a symlink: $root/$name"
    fi
    if [[ ! -f $root/$name/SKILL.md || -L $root/$name/SKILL.md ]]; then
      skill_fail "skill is missing a regular SKILL.md: $root/$name"
    fi
    names+=("$name")
  done
  shopt -u nullglob
  if [[ ${#names[@]} -eq 0 ]]; then
    skill_fail "no skills in $root"
  fi
  printf '%s\n' "${names[@]}" | sort
}

skill_file_mode() {
  if stat -c %a "$1" >/dev/null 2>&1; then
    stat -c %a "$1"
  else
    stat -f %OLp "$1"
  fi
}

# Directories are 755. A file keeps the executable bit and is otherwise 644.
# Nix store paths drop the owner-write bit (755 -> 555, 644 -> 444); the
# installed tree has to stay writable or the next run cannot repair it.
skill_dest_mode() {
  local path=$1
  local mode
  if [[ -d $path && ! -L $path ]]; then
    printf '755\n'
    return
  fi
  mode=$(skill_file_mode "$path")
  if [[ $((8#$mode & 0111)) -ne 0 ]]; then
    printf '755\n'
  else
    printf '644\n'
  fi
}

skill_mode_ok() {
  [[ $(skill_file_mode "$2") == "$(skill_dest_mode "$1")" ]]
}

skill_apply_mode() {
  chmod "$(skill_dest_mode "$1")" "$2"
}

# Nix drops the owner-write bit, so 755 and 555 are the same executable file.
skill_is_exec() {
  local mode
  mode=$(skill_file_mode "$1")
  [[ $((8#$mode & 0111)) -ne 0 ]]
}

skill_exec_ok() {
  local src_exec=1 dest_exec=1
  if [[ -d $1 && ! -L $1 ]]; then
    return 0
  fi
  skill_is_exec "$1" || src_exec=0
  skill_is_exec "$2" || dest_exec=0
  [[ $src_exec -eq $dest_exec ]]
}

skill_assert_source_file() {
  local path=$1
  if [[ -L $path ]]; then
    skill_fail "source symlink: $path"
  fi
  if [[ -d $path || -f $path ]]; then
    return 0
  fi
  skill_fail "unsupported source file: $path"
}

# mode is apply or check. apply rewrites dest until it matches src.
# keep_mode=1 leaves an existing directory or file mode alone. A file that
# already exists keeps that mode after a byte rewrite. New paths still get
# the normalized mode.
skill_sync_tree() {
  local src=$1 dest=$2 mode=$3 keep_mode=${4:-0}
  local rel parent tmp preserved dest_ready nested_ready
  if [[ $mode != apply && $mode != check ]]; then
    skill_fail "unknown sync mode: $mode"
  fi
  skill_assert_source_file "$src"
  if [[ ! -d $src ]]; then
    skill_fail "source skill is not a directory: $src"
  fi

  dest_ready=0
  if [[ -d $dest && ! -L $dest ]]; then
    dest_ready=1
  fi
  if [[ -L $dest ]]; then
    if [[ $mode == check ]]; then
      skill_fail "symlink: $dest"
    fi
    rm -f -- "$dest"
  elif [[ -e $dest && ! -d $dest ]]; then
    if [[ $mode == check ]]; then
      skill_fail "not a directory: $dest"
    fi
    rm -f -- "$dest"
  fi
  if [[ ! -d $dest ]]; then
    if [[ $mode == check ]]; then
      skill_fail "missing directory: $dest"
    fi
    mkdir -p -- "$dest"
  fi
  if [[ $mode == apply && ( $keep_mode == 0 || $dest_ready == 0 ) ]] && ! skill_mode_ok "$src" "$dest"; then
    skill_apply_mode "$src" "$dest"
  fi

  while IFS= read -r -d '' rel; do
    rel=${rel#./}
    skill_assert_source_file "$src/$rel"
    if [[ -d $src/$rel ]]; then
      nested_ready=0
      if [[ -d $dest/$rel && ! -L $dest/$rel ]]; then
        nested_ready=1
      fi
      if [[ -L $dest/$rel ]]; then
        if [[ $mode == check ]]; then
          skill_fail "symlink: $dest/$rel"
        fi
        rm -f -- "$dest/$rel"
      elif [[ -e $dest/$rel && ! -d $dest/$rel ]]; then
        if [[ $mode == check ]]; then
          skill_fail "not a directory: $dest/$rel"
        fi
        rm -f -- "$dest/$rel"
      fi
      if [[ ! -d $dest/$rel ]]; then
        if [[ $mode == check ]]; then
          skill_fail "missing directory: $dest/$rel"
        fi
        mkdir -p -- "$dest/$rel"
      fi
      if [[ $mode == apply && ( $keep_mode == 0 || $nested_ready == 0 ) ]] && ! skill_mode_ok "$src/$rel" "$dest/$rel"; then
        skill_apply_mode "$src/$rel" "$dest/$rel"
      fi
    else
      if [[ -L $dest/$rel ]]; then
        if [[ $mode == check ]]; then
          skill_fail "symlink: $dest/$rel"
        fi
      elif [[ ! -f $dest/$rel ]]; then
        if [[ $mode == check ]]; then
          skill_fail "missing file: $dest/$rel"
        fi
      elif ! cmp -s -- "$src/$rel" "$dest/$rel"; then
        if [[ $mode == check ]]; then
          skill_fail "byte drift: $dest/$rel"
        fi
      elif [[ $keep_mode == 0 ]] && ! skill_exec_ok "$src/$rel" "$dest/$rel"; then
        if [[ $mode == check ]]; then
          skill_fail "mode drift: $dest/$rel"
        fi
        skill_apply_mode "$src/$rel" "$dest/$rel"
        continue
      elif [[ $mode == apply && $keep_mode == 0 ]] && ! skill_mode_ok "$src/$rel" "$dest/$rel"; then
        skill_apply_mode "$src/$rel" "$dest/$rel"
        continue
      else
        continue
      fi
      preserved=""
      if [[ $keep_mode == 1 && -f $dest/$rel && ! -L $dest/$rel ]]; then
        preserved=$(skill_file_mode "$dest/$rel")
      fi
      parent=$(dirname -- "$dest/$rel")
      mkdir -p -- "$parent"
      rm -f -- "$dest/$rel"
      tmp=$(mktemp "$parent/.bezel-skill.XXXXXX")
      cat -- "$src/$rel" >"$tmp"
      mv -f -- "$tmp" "$dest/$rel"
      if [[ -n $preserved ]]; then
        chmod "$preserved" "$dest/$rel"
      else
        skill_apply_mode "$src/$rel" "$dest/$rel"
      fi
    fi
  done < <(cd -- "$src" && find . -mindepth 1 -print0)

  local -a extras=()
  while IFS= read -r -d '' rel; do
    rel=${rel#./}
    if [[ ! -e $src/$rel && ! -L $src/$rel ]]; then
      extras+=("$dest/$rel")
    fi
  done < <(cd -- "$dest" && find . -depth -mindepth 1 -print0)
  if [[ ${#extras[@]} -gt 0 ]]; then
    if [[ $mode == check ]]; then
      skill_fail "extra path: ${extras[0]}"
    fi
    for rel in "${extras[@]}"; do
      [[ -e $rel || -L $rel ]] && rm -rf -- "$rel"
    done
  fi
}

# A skills root that must contain exactly the canonical skill directories.
skill_sync_root() {
  local src=$1 dest=$2 mode=$3
  local name
  local -a names=()
  mapfile -t names < <(skill_dir_names "$src")
  if [[ -L $dest ]]; then
    if [[ $mode == check ]]; then
      skill_fail "skills directory is a symlink: $dest"
    fi
    rm -f -- "$dest"
  elif [[ -e $dest && ! -d $dest ]]; then
    if [[ $mode == check ]]; then
      skill_fail "skills path is not a directory: $dest"
    fi
    rm -f -- "$dest"
  fi
  if [[ ! -d $dest ]]; then
    if [[ $mode == check ]]; then
      skill_fail "missing skills directory: $dest"
    fi
    mkdir -p -- "$dest"
  fi
  for name in "${names[@]}"; do
    skill_sync_tree "$src/$name" "$dest/$name" "$mode"
  done
  local base found have
  while IFS= read -r -d '' base; do
    base=$(basename -- "$base")
    found=0
    for have in "${names[@]}"; do
      if [[ $have == "$base" ]]; then
        found=1
        break
      fi
    done
    if [[ $found -eq 0 ]]; then
      if [[ $mode == check ]]; then
        skill_fail "extra path: $dest/$base"
      fi
      rm -rf -- "$dest/$base"
    fi
  done < <(find "$dest" -mindepth 1 -maxdepth 1 -print0)
}

# User install: write each canonical skill, and leave unrelated skill names in place.
skill_sync_user_root() {
  local src=$1 dest=$2 mode=$3
  local name
  local -a names=()
  mapfile -t names < <(skill_dir_names "$src")
  if [[ -L $dest ]]; then
    if [[ $mode == check ]]; then
      skill_fail "skills directory is a symlink: $dest"
    fi
    rm -f -- "$dest"
  elif [[ -e $dest && ! -d $dest ]]; then
    if [[ $mode == check ]]; then
      skill_fail "skills path is not a directory: $dest"
    fi
    rm -f -- "$dest"
  fi
  if [[ ! -d $dest ]]; then
    if [[ $mode == check ]]; then
      skill_fail "missing skills directory: $dest"
    fi
    mkdir -p -- "$dest"
  fi
  for name in "${names[@]}"; do
    skill_sync_tree "$src/$name" "$dest/$name" "$mode" 1
  done
}

# User install must not overwrite a regular file whose bytes differ, or replace
# a symlink, unless the caller passes --force. Prints every conflict.
# Returns 1 when any exist. A missing file is not a conflict.
skill_user_conflicts() {
  local src=$1 dest=$2
  local name rel path status=0
  local -a names=()
  if [[ ! -e $dest || -L $dest || ! -d $dest ]]; then
    return 0
  fi
  mapfile -t names < <(skill_dir_names "$src")
  for name in "${names[@]}"; do
    if [[ ! -d $dest/$name || -L $dest/$name ]]; then
      continue
    fi
    while IFS= read -r -d '' rel; do
      rel=${rel#./}
      [[ -f $src/$name/$rel && ! -L $src/$name/$rel ]] || continue
      path=$dest/$name/$rel
      if [[ -L $path ]]; then
        printf 'install-user-skills: symlink: %s\n' "$path" >&2
        status=1
      elif [[ -f $path ]]; then
        if ! cmp -s -- "$src/$name/$rel" "$path"; then
          printf 'install-user-skills: byte drift: %s\n' "$path" >&2
          status=1
        fi
      elif [[ -e $path ]]; then
        printf 'install-user-skills: not a regular file: %s\n' "$path" >&2
        status=1
      fi
    done < <(cd -- "$src/$name" && find . -mindepth 1 -type f -print0)
  done
  return "$status"
}
