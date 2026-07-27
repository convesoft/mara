#!/bin/sh
# ART-SCALE-V01-VERIFIER: a fixed, independent POSIX oracle for scale-v0.1.
set -u
LC_ALL=C
export LC_ALL

usage() {
    printf '%s\n' 'usage: tests/qualification/verify-scale-v01.sh --qualification-root ABSOLUTE_PATH' >&2
    exit 64
}

if [ "$#" -ne 2 ] || [ "$1" != '--qualification-root' ]; then
    usage
fi
case $2 in
    /*) ;;
    *) usage ;;
esac

repo_from_git=$(git rev-parse --show-toplevel 2>/dev/null) || {
    printf '%s\n' 'verifier must run inside the source Git repository' >&2
    exit 65
}
repo=$(CDPATH= cd -- "$repo_from_git" && pwd -P) || exit 65
here=$(pwd -P) || exit 65
if [ "$here" != "$repo" ]; then
    printf '%s\n' 'verifier must run from the canonical source repository root' >&2
    exit 65
fi

root=$(CDPATH= cd -- "$2" && pwd -P) || {
    printf '%s\n' 'qualification root must exist and be canonicalizable' >&2
    exit 66
}
fixture=$root/fixture
evidence=$root/evidence
manifest=$repo/tests/qualification/scale-v01.SHA256SUMS
mara=$repo/target/release/mara
if [ ! -d "$fixture" ] || [ -L "$fixture" ] || [ ! -d "$evidence" ] || [ -L "$evidence" ]; then
    printf '%s\n' 'qualification root must contain real fixture and evidence directories' >&2
    exit 66
fi
if [ ! -f "$manifest" ] || [ -L "$manifest" ] || [ ! -f "$mara" ] || [ -L "$mara" ]; then
    printf '%s\n' 'expected manifest and release mara executable must be regular files' >&2
    exit 66
fi

for output in \
    file-set-check.txt file-set-check.stderr file-type-check.txt \
    manifest-path-check.txt manifest-path-check.stderr fixture-git-context.stdout \
    fixture-git-context.stderr fixture-git-context.txt fixture-sha256-check.txt \
    count-check.txt count-check.stderr topology-check.txt preflight-check.json \
    preflight-check.stderr preflight-check-exit.txt
do
    if [ -e "$evidence/$output" ] || [ -L "$evidence/$output" ]; then
        printf '%s\n' "oracle output already exists: $output" >&2
        exit 66
    fi
done

case $(uname -s) in
    Linux)
        manifest_digest() { sha256sum "$manifest" | awk '{print $1}'; }
        fixture_digest() { sha256sum "$1" | awk '{print $1}'; }
        verify_digests() { (CDPATH= cd -- "$fixture" && sha256sum --check "$manifest"); }
        ;;
    Darwin)
        manifest_digest() { shasum -a 256 "$manifest" | awk '{print $1}'; }
        fixture_digest() { shasum -a 256 "$1" | awk '{print $1}'; }
        verify_digests() { (CDPATH= cd -- "$fixture" && shasum -a 256 -c "$manifest"); }
        ;;
    *)
        manifest_digest() { return 1; }
        fixture_digest() { return 1; }
        verify_digests() { return 1; }
        ;;
esac

failed=0

expected_entries='.mara
.mara/project.toml
.mara/schema.yaml
items-000.mara.md
items-001.mara.md
items-002.mara.md
items-003.mara.md
items-004.mara.md
items-005.mara.md
items-006.mara.md
items-007.mara.md
items-008.mara.md
items-009.mara.md'
actual_entries=$(CDPATH= cd -- "$fixture" && find . -mindepth 1 -print 2>"$evidence/file-set-check.stderr" | sed 's|^\./||' | sort) || failed=1
{
    printf '%s\n' 'expected:'
    printf '%s\n' "$expected_entries"
    printf '%s\n' 'actual:'
    printf '%s\n' "$actual_entries"
} >"$evidence/file-set-check.txt"
if [ "$actual_entries" != "$expected_entries" ]; then
    failed=1
fi

{
    type_ok=1
    if [ ! -d "$fixture" ] || [ -L "$fixture" ]; then
        type_ok=0
    fi
    if [ ! -d "$fixture/.mara" ] || [ -L "$fixture/.mara" ]; then
        type_ok=0
    fi
    for path in .mara/project.toml .mara/schema.yaml items-000.mara.md items-001.mara.md \
        items-002.mara.md items-003.mara.md items-004.mara.md items-005.mara.md \
        items-006.mara.md items-007.mara.md items-008.mara.md items-009.mara.md
    do
        if [ ! -f "$fixture/$path" ] || [ -L "$fixture/$path" ]; then
            type_ok=0
            printf 'invalid_type=%s\n' "$path"
        fi
    done
    if [ "$type_ok" -eq 1 ]; then
        printf '%s\n' 'file_types=ok'
    else
        printf '%s\n' 'file_types=failed'
    fi
} >"$evidence/file-type-check.txt"
if ! grep -qx 'file_types=ok' "$evidence/file-type-check.txt"; then
    failed=1
fi

manifest_hash=$(manifest_digest 2>>"$evidence/manifest-path-check.stderr") || manifest_hash=
manifest_final_lf=0
if [ -s "$manifest" ] && [ "$(tail -c 1 "$manifest" | od -An -tu1 | tr -d '[:space:]')" = '10' ]; then
    manifest_final_lf=1
fi
{
    printf 'manifest_sha256=%s\n' "$manifest_hash"
    awk -v manifest_final_lf="$manifest_final_lf" '
        BEGIN {
            expected[1] = ".mara/project.toml";
            expected[2] = ".mara/schema.yaml";
            for (i = 0; i < 10; i++) expected[i + 3] = sprintf("items-%03d.mara.md", i);
            ok = 1;
        }
        {
            digest = substr($0, 1, 64);
            spaces = substr($0, 65, 2);
            path = substr($0, 67);
            if (length($0) != 66 + length(path) || length(digest) != 64 || digest !~ /^[0-9a-f]+$/ || spaces != "  " || path == "" || path ~ /[[:space:]]/ || path != expected[NR]) {
                ok = 0;
                printf "manifest_error=line_%d\n", NR;
                next;
            }
            seen[path]++;
            printf "manifest_entry=%s %s\n", path, digest;
        }
        END {
            if (manifest_final_lf != 1) { ok = 0; printf "manifest_error=missing_final_lf\n"; }
            if (NR != 12) { ok = 0; printf "manifest_error=line_count\n"; }
            for (i = 1; i <= 12; i++) if (seen[expected[i]] != 1) { ok = 0; }
            if (ok) printf "manifest_syntax=ok\n"; else printf "manifest_syntax=failed\n";
            exit(ok ? 0 : 1);
        }
    ' "$manifest"
} >"$evidence/manifest-path-check.txt" 2>>"$evidence/manifest-path-check.stderr" || failed=1
if ! grep -qx 'manifest_syntax=ok' "$evidence/manifest-path-check.txt"; then
    failed=1
fi

(
    digest_ok=1
    verify_digests || digest_ok=0
    for path in .mara/project.toml .mara/schema.yaml items-000.mara.md items-001.mara.md \
        items-002.mara.md items-003.mara.md items-004.mara.md items-005.mara.md \
        items-006.mara.md items-007.mara.md items-008.mara.md items-009.mara.md
    do
        expected_sha256=$(awk -v expected_path="$path" '$2 == expected_path { print $1 }' "$manifest")
        observed_sha256=$(fixture_digest "$fixture/$path" 2>/dev/null) || observed_sha256=
        matched=false
        if [ "$expected_sha256" = "$observed_sha256" ] && [ -n "$expected_sha256" ]; then
            matched=true
        else
            digest_ok=0
        fi
        printf 'path=%s expected_sha256=%s observed_sha256=%s matched=%s\n' \
            "$path" "$expected_sha256" "$observed_sha256" "$matched"
    done
    if [ "$digest_ok" -eq 1 ]; then
        printf '%s\n' 'digest_check=ok'
    else
        printf '%s\n' 'digest_check=failed'
        exit 1
    fi
) >"$evidence/fixture-sha256-check.txt" 2>&1 || failed=1
if ! grep -qx 'digest_check=ok' "$evidence/fixture-sha256-check.txt"; then
    failed=1
fi

set +e
git -C "$fixture" rev-parse --show-toplevel >"$evidence/fixture-git-context.stdout" 2>"$evidence/fixture-git-context.stderr"
fixture_git_status=$?
set -e
printf 'exit_code=%s\n' "$fixture_git_status" >"$evidence/fixture-git-context.txt"
if [ "$fixture_git_status" -eq 0 ]; then
    failed=1
fi

{
    item_count=$(grep -h '^:id: SCALE-[0-9][0-9][0-9][0-9][0-9]$' "$fixture"/items-*.mara.md | wc -l | tr -d ' ')
    edge_count=$(grep -h '^:depends_on: SCALE-[0-9][0-9][0-9][0-9][0-9]$' "$fixture"/items-*.mara.md | wc -l | tr -d ' ')
    printf 'items=%s\n' "$item_count"
    printf 'edges=%s\n' "$edge_count"
} >"$evidence/count-check.txt" 2>"$evidence/count-check.stderr"
if ! grep -qx 'items=10000' "$evidence/count-check.txt" || ! grep -qx 'edges=100000' "$evidence/count-check.txt"; then
    failed=1
fi

awk '
    function fail(message) { print "topology_error=" message; ok = 0; }
    BEGIN { ok = 1; source = -1; item_count = 0; }
    /^:id: SCALE-[0-9][0-9][0-9][0-9][0-9]$/ {
        if (source >= 0 && edge_count != 10) fail("edge_count");
        source = substr($2, 7) + 0;
        if (seen_source[source]++) fail("duplicate_source");
        item_count++;
        edge_count = 0;
        next;
    }
    /^:depends_on: SCALE-[0-9][0-9][0-9][0-9][0-9]$/ {
        if (source < 0) { fail("relation_before_source"); next; }
        edge_count++;
        target = substr($2, 7) + 0;
        expected = (source + edge_count) % 10000;
        if (target != expected) fail("unexpected_target");
        if (seen_target[source ":" target]++) fail("duplicate_target");
        next;
    }
    END {
        if (source >= 0 && edge_count != 10) fail("edge_count");
        if (item_count != 10000) fail("item_count");
        for (i = 0; i < 10000; i++) if (!seen_source[i]) fail("missing_source");
        if (ok) print "topology=ok"; else print "topology=failed";
        exit(ok ? 0 : 1);
    }
' "$fixture"/items-*.mara.md >"$evidence/topology-check.txt" || failed=1
if ! grep -qx 'topology=ok' "$evidence/topology-check.txt"; then
    failed=1
fi

set +e
(CDPATH= cd -- "$fixture" && "$mara" check --format json) >"$evidence/preflight-check.json" 2>"$evidence/preflight-check.stderr"
preflight_status=$?
set -e
printf 'exit_code=%s\n' "$preflight_status" >"$evidence/preflight-check-exit.txt"
if [ "$preflight_status" -ne 0 ] || \
    ! grep -q '"status": "ok"' "$evidence/preflight-check.json" || \
    ! grep -q '"diagnostics": \[\]' "$evidence/preflight-check.json" || \
    ! grep -q '"documents": 10' "$evidence/preflight-check.json" || \
    ! grep -q '"items": 10000' "$evidence/preflight-check.json" || \
    ! grep -q '"edges": 100000' "$evidence/preflight-check.json"
then
    failed=1
fi

if [ "$failed" -ne 0 ]; then
    exit 1
fi
