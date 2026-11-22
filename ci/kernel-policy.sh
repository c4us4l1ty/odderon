#!/usr/bin/env bash
# Kernel policy: the laws that can be enforced by grep.
#
# Each check corresponds to a rule in docs/VISION.md §3 and docs/DETERMINISM.md §1.3. These are not
# style preferences — every one of them, if violated, makes a replay unverifiable or an architecture
# promise false.
#
# Written as a script rather than inline YAML for two reasons: the patterns are legible, and the
# script can be run locally (`bash ci/kernel-policy.sh`) so a rule is testable before it is enforced.
#
# Exit 0 if clean, 1 on any violation.
set -uo pipefail

fail=0

report() {
  # report <rule> <hits>
  echo "::error::$1"
  echo "$2"
  fail=1
}

# ---------------------------------------------------------------------------------------------
# Rule 1 — transcendentals come from odderon-dmath, never from the host.
#
# Excluded deliberately: the dmath crate itself (it *is* the implementation) and the test module
# inside it (which compares against the platform on purpose).
# ---------------------------------------------------------------------------------------------
rule1_hits=""
while IFS= read -r line; do
  [ -z "$line" ] && continue
  file="${line%%:*}"
  if [ "$file" = "crates/odderon-dmath/src/lib.rs" ]; then
    # Allowed only below the `#[cfg(test)]` marker.
    lineno="${line#*:}"; lineno="${lineno%%:*}"
    test_start=$(grep -n '^#\[cfg(test)\]' "$file" | head -1 | cut -d: -f1)
    if [ -n "$test_start" ] && [ "$lineno" -ge "$test_start" ]; then
      continue
    fi
  fi
  rule1_hits="${rule1_hits}${line}"$'\n'
done < <(grep -rnE '\.(sin|cos|tan|asin|acos|atan|atan2|exp|exp2|expm1|ln|log|log2|log10|powf|powi|cbrt|hypot|sinh|cosh|tanh|to_degrees|sqrt|abs|fract|recip)\(' \
           crates --include='*.rs' \
         | grep -vE '^[^:]+:[0-9]+:[[:space:]]*(//|/\*|\*)' || true)

if [ -n "$rule1_hits" ]; then
  report "Host/float method used in the kernel. Use odderon_dmath (sin, cos, ln, atan2, sqrt, fabs, fmax, ...)." "$rule1_hits"
else
  echo "ok  R1  no host transcendentals outside odderon-dmath"
fi

# ---------------------------------------------------------------------------------------------
# Rule 2 — time is an argument, never a query.
# ---------------------------------------------------------------------------------------------
rule2_hits=$(grep -rnE 'Instant::now|SystemTime::now|std::time|core::time' crates --include='*.rs' \
             | grep -vE '^[^:]+:[0-9]+:[[:space:]]*(//|/\*|\*)' || true)
if [ -n "$rule2_hits" ]; then
  report "Wall clock read in the kernel. Time is an argument (t: i64 microseconds since J2000 TDB)." "$rule2_hits"
else
  echo "ok  R2  kernel has no clock"
fi

# ---------------------------------------------------------------------------------------------
# Rule 3 — no hash-order dependence.
# ---------------------------------------------------------------------------------------------
rule3_hits=$(grep -rnE '\b(HashMap|HashSet)\b' crates --include='*.rs' \
             | grep -vE '^[^:]+:[0-9]+:[[:space:]]*(//|/\*|\*)' || true)
if [ -n "$rule3_hits" ]; then
  report "HashMap/HashSet iteration order is not deterministic. Use BTreeMap or indexed vectors." "$rule3_hits"
else
  echo "ok  R3  no HashMap in the kernel"
fi

# ---------------------------------------------------------------------------------------------
# Rule 4 — the kernel never touches the presentation layer.
# ---------------------------------------------------------------------------------------------
rule4_hits=$(grep -rnE 'odderon_(present|shell)|web_sys|js_sys|wasm_bindgen' crates --include='*.rs' \
             | grep -vE '^crates/(odderon-present|odderon-shell)/' \
             | grep -vE '^[^:]+:[0-9]+:[[:space:]]*(//|/\*|\*)' || true)
if [ -n "$rule4_hits" ]; then
  report "Kernel crate touched the presentation layer, which breaks determinism and Proof-of-Flight." "$rule4_hits"
else
  echo "ok  R4  kernel/presenter boundary intact"
fi

# ---------------------------------------------------------------------------------------------
# Rule 5 — zero binary assets (GRAPHICS.md R1).
# ---------------------------------------------------------------------------------------------
rule5_hits=$(git ls-files | grep -iE '\.(png|jpe?g|webp|gif|bmp|tga|dds|ktx2?|basis|obj|fbx|gltf|glb|dae|3ds|blend)$' || true)
if [ -n "$rule5_hits" ]; then
  report "Binary asset found. Every surface must be a function S(u,v,t) — see GRAPHICS.md R1." "$rule5_hits"
else
  echo "ok  R5  zero binary assets"
fi

# ---------------------------------------------------------------------------------------------
# Rule 6 — `no_std` kernel: no accidental std dependence.
# ---------------------------------------------------------------------------------------------
rule6_hits=$(grep -rnE 'extern crate std|use std::' crates/odderon-dmath/src crates/odderon-*/src \
             --include='*.rs' 2>/dev/null \
             | grep -vE '(dmath-wasm|wasm)' \
             | awk -F: '{
                 file=$1; line=$2;
                 cmd = "grep -n \"^#\\\\[cfg(test)\\\\]\" " file " | head -1 | cut -d: -f1";
                 cmd | getline ts; close(cmd);
                 if (ts == "" || line < ts) print $0
               }' || true)
if [ -n "$rule6_hits" ]; then
  report "std used in a kernel crate outside a test module. Kernel crates are no_std." "$rule6_hits"
else
  echo "ok  R6  kernel crates are no_std"
fi

if [ "$fail" -ne 0 ]; then
  echo ""
  echo "Kernel policy violated. See docs/DETERMINISM.md §1.3 for why each rule exists."
  exit 1
fi

echo ""
echo "kernel policy: all rules pass"
