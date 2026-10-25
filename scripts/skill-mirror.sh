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
skill_sync_tree() {
  local src=$1 dest=$2 mode=$3
  local rel parent tmp
  if [[ $mode != apply && $mode != check ]]; then
    skill_fail "unknown sync mode: $mode"
  fi
  skill_assert_source_file "$src"
  if [[ ! -d $src ]]; then
    skill_fail "source skill is not a directory: $src"
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

  while IFS= read -r -d '' rel; do
    rel=${rel#./}
    skill_assert_source_file "$src/$rel"
    if [[ -d $src/$rel ]]; then
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
      else
        continue
      fi
      parent=$(dirname -- "$dest/$rel")
      mkdir -p -- "$parent"
      rm -f -- "$dest/$rel"
      tmp=$(mktemp "$parent/.bezel-skill.XXXXXX")
      cat -- "$src/$rel" >"$tmp"
      mv -f -- "$tmp" "$dest/$rel"
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
    skill_sync_tree "$src/$name" "$dest/$name" "$mode"
  done
}
