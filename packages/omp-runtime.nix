{ writeShellApplication }:

# Fail-closed PATH / OMP_BIN resolver. Not omp source.
# Used as the wrapProgram target when the 180–240MB fetchurl pin is not built.
writeShellApplication {
  name = "omp";
  text = ''
    if [ -n "''${OMP_BIN:-}" ]; then
      if [ ! -x "$OMP_BIN" ]; then
        echo "omapi-overlay: OMP_BIN=$OMP_BIN is not executable (fail closed)." >&2
        exit 127
      fi
      exec "$OMP_BIN" "$@"
    fi

    self="$(readlink -f "$0" 2>/dev/null || printf '%s' "$0")"
    found=""
    IFS=':'
    for dir in $PATH; do
      [ -n "$dir" ] || continue
      cand="$dir/omp"
      if [ -x "$cand" ]; then
        real="$(readlink -f "$cand" 2>/dev/null || printf '%s' "$cand")"
        if [ "$real" != "$self" ]; then
          found="$cand"
          break
        fi
      fi
    done
    unset IFS

    if [ -z "$found" ]; then
      echo "omapi-overlay: stock omp not found." >&2
      echo "Install omp (https://omp.sh) or set OMP_BIN to the upstream binary." >&2
      echo "This overlay does not vendor omp source. See README (runtime wrap vs omapi-pinned)." >&2
      exit 127
    fi
    exec "$found" "$@"
  '';
}
