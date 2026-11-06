#!/usr/bin/env bash
# Exercise the Fish wire format without requiring a shell configuration or an engine.
set -eu
bin=${1:-delonix}
check_pair() {
    local group=$1 canonical=$2 alias=$3 candidates primary alternate
    local -a words=(delonix)
    if [[ -n $group ]]; then words+=("$group"); fi
    candidates=$(COMPLETE=fish "$bin" -- "${words[@]}" '')
    primary=$(awk -F '\t' -v cmd="$canonical" '$1 == cmd {print $2}' <<<"$candidates")
    alternate=$(awk -F '\t' -v cmd="$alias" '$1 == cmd {print $2}' <<<"$candidates")
    if [[ -z $primary || $primary != "$alternate" ]]; then
        echo "FAIL: expected $group {$canonical,$alias} with matching descriptions" >&2
        return 1
    fi
}
check_pair '' ps ls
check_pair container ps ls
check_pair image remove rm
echo 'PASS: all three command pairs have matching TAB descriptions'
