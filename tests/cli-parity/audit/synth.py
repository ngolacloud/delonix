"""Type-aware synthetic values for option arguments.

Why this exists: the first pass of the matrix passed the string `aud-synth` to
every option that takes a value, and three cases came back FAIL for the wrong
reason — clap validated the VALUE before it reached the conflict, so
`--max-for-day aud-synth` answered "invalid digit" and the conflict the case
existed to prove was never evaluated. A case that cannot reach the rule it
tests proves nothing, so the value has to parse.

The type is inferred from the value name and the description, in this order,
and the inference is deliberately conservative: anything unrecognised keeps the
plain string, which is correct for the majority (names, tags, labels).
"""
from __future__ import annotations

import re

NUMERIC_NAME = re.compile(
    r"^(N|NUM|COUNT|VCPUS?|VCPUS_MAX|REPLICAS|PORT|PID|UID|GID|WEIGHT|"
    r"SHARES|THRESHOLD|PCT|PERCENT|LIMIT|RETRIES|INDEX|SERIAL|VNI|GENERATION)$"
)
DURATION_NAME = re.compile(r"(TIMEOUT|TTL|INTERVAL|DURATION|PERIOD|DELAY|GRACE|SINCE)$")
PATH_NAME = re.compile(r"(PATH|FILE|DIR|KEY|CERT|CA_CERT|CA_KEY|SRC|DEST|CONFIG)$")
CIDR_NAME = re.compile(r"(CIDR|SUBNET|PREFIX)$")
IP_NAME = re.compile(r"^(IP|GATEWAY|ADDRESS|NODE_IP|WG_IP)$")
ADDR_NAME = re.compile(r"^(ADDR|ENDPOINT|URL|SERVER|URI)$")
SIZE_NAME = re.compile(r"^(SIZE|MEMORY|MEMORY_MAX|QUOTA|SHM_SIZE|BPS|IOPS)$")


def synth_value(value_name: str | None, description: str, *,
                existing_file: str | None = None) -> str:
    """A value that PARSES for this option, so the case reaches its rule."""
    vn = (value_name or "").strip().upper().replace("-", "_")
    desc = (description or "").lower()

    if NUMERIC_NAME.match(vn):
        return "1"
    if DURATION_NAME.search(vn):
        return "30s"
    if CIDR_NAME.search(vn):
        return "10.231.0.0/24"
    if IP_NAME.match(vn):
        return "10.231.0.2"
    if SIZE_NAME.match(vn):
        return "64m"
    if ADDR_NAME.match(vn):
        # The engine refuses anything that is not a transport URL on these.
        if "tcp://" in desc or "must be tcp" in desc:
            return "tcp://127.0.0.1:19999"
        if "unix://" in desc:
            return "unix:///tmp/aud.sock"
        return "tcp://127.0.0.1:19999"
    if PATH_NAME.search(vn) and existing_file:
        return existing_file

    # Descriptions that name a shape even when the value name does not.
    if re.search(r"\bkey=value\b|\bKEY=VALUE\b", description):
        return "AUD_K=v"
    if "digits and dots" in desc or "kubernetes version" in desc:
        return "1.32.1"
    if re.search(r"\bseconds\b.*\bunit\b|\b500ms\b", desc):
        return "30s"
    if re.search(r"\bcidr\b", desc):
        return "10.231.0.0/24"
    if re.search(r"\bnumber\b|\binteger\b|\bcount\b", desc) and len(vn) <= 3:
        return "1"

    return "aud-synth"


# Source module -> the command paths that module serves. Used to resolve a
# DECLARED conflict to the leaf it actually governs: resolving by "the leaf
# publishes both flags" over-matched, and claimed `network create --driver
# --parent` was a conflict when the rule lives in volume.rs.
MODULE_PATHS: dict[str, tuple[str, ...]] = {
    "volume.rs": ("volume", "volumes"),
    "vm.rs": ("vm",),
    "container.rs": ("container", "run", "create", "exec", "start", "stop",
                     "restart", "rm", "logs", "kill", "attach", "stats", "cp",
                     "port", "pause", "unpause", "commit", "top", "rename",
                     "update", "wait"),
    "system.rs": ("system",),
    "netns.rs": ("net",),
    "hosts.rs": ("hosts",),
    "man.rs": ("man",),
    "rbackup.rs": ("backup",),
    "network.rs": ("network",),
    "pod.rs": ("pod",),
    "image.rs": ("image", "images", "pull", "push", "tag", "save", "load",
                 "rmi", "login", "logout", "history", "untag"),
}


def module_matches(file: str, path_str: str) -> bool:
    base = file.rsplit("/", 1)[-1]
    heads = MODULE_PATHS.get(base)
    if not heads:
        return True  # unknown module: do not over-restrict
    head = path_str.split()[0]
    return head in heads
