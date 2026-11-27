//! The `VMfile` directives that decide what an image does NOT carry —
//! `SLIM`, `INSTALL`, `REMOVE`, `AUTOSTART` and `BUDGET` — and the measurement
//! that proves it stayed that way.
//!
//! They live apart from the grammar in `cmd::vmfile` because they share one
//! idea the rest of that file does not: **the image is what was declared.**
//! A cloud image starts with everything its publisher thought someone might
//! want; the cheap way to make it small is to stop things from entering
//! (`SLIM`, before any package is installed), install by name (`INSTALL`),
//! prune what still came in (`REMOVE`), leave running only what was asked for
//! (`AUTOSTART --only`) and then measure the result against a number
//! (`BUDGET`). Removing after the fact alone is the expensive way, and a size
//! nobody measures does not shrink.
//!
//! **One recipe, either package manager.** A `VMfile` names a disk, not a
//! distro: `FROM https://…` or a local image says nothing about apt or dnf. So
//! the choice is made INSIDE the guest, by what it has ([`pm_switch`]), and a
//! guest with neither fails the step instead of skipping it — the same
//! fail-closed rule as the parser.

use delonix_model::{Error, Result};

use super::vmimage::CustomizeOp;

/// `SLIM` — what the package manager must not install from here on. Each
/// field is `None` when the directive did not mention it (nothing changes).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Slim {
    /// `--docs=off`: no `/usr/share/{doc,man,info}` (copyright files stay).
    pub(crate) docs: Option<bool>,
    /// `--recommends=off`: apt Recommends/Suggests, dnf weak dependencies.
    pub(crate) recommends: Option<bool>,
    /// `--locales=pt,en`: the message catalogs to keep; every other language
    /// under `/usr/share/locale` is excluded.
    pub(crate) locales: Option<Vec<String>>,
}

/// `REMOVE` — one positional removal step.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Removal {
    pub(crate) packages: Vec<String>,
    pub(crate) paths: Vec<String>,
    /// Disabled AND masked, so nothing pulls them back in.
    pub(crate) services: Vec<String>,
}

/// `AUTOSTART` — accumulated over every line of a stage, applied once at its
/// end (see [`autostart_ops`] for why the end).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Autostart {
    /// Normalized unit names (`smbd` → `smbd.service`), in declaration order.
    pub(crate) units: Vec<String>,
    /// `--only`: every OTHER enabled unit is disabled.
    pub(crate) only: bool,
}

// ─── parsing ────────────────────────────────────────────────────────────────

/// Splits `--name=value` flags. Anything that is not a flag is an error: these
/// directives take no positional words, and a stray word is a typo.
fn flags(rest: &str, directive: &str) -> std::result::Result<Vec<(String, String)>, String> {
    rest.split_whitespace()
        .map(|w| {
            let body = w.strip_prefix("--").ok_or_else(|| {
                format!("{directive} takes only --flag=value arguments, got '{w}'")
            })?;
            let (k, v) = body
                .split_once('=')
                .ok_or_else(|| format!("{directive} --{body} needs a value (--{body}=…)"))?;
            Ok((k.to_string(), v.to_string()))
        })
        .collect()
}

fn on_off(directive: &str, k: &str, v: &str) -> std::result::Result<bool, String> {
    match v {
        "on" => Ok(true),
        "off" => Ok(false),
        _ => Err(format!("{directive} --{k} is on or off, got '{v}'")),
    }
}

/// A language code as it names a directory under `/usr/share/locale`:
/// `pt`, `pt_BR`, `ast`. Strict, because it is interpolated into a shell
/// `case` pattern and a dpkg path filter.
fn valid_locale(code: &str) -> bool {
    let (lang, region) = match code.split_once('_') {
        Some((l, r)) => (l, Some(r)),
        None => (code, None),
    };
    (2..=3).contains(&lang.len())
        && lang.chars().all(|c| c.is_ascii_lowercase())
        && region.is_none_or(|r| r.len() == 2 && r.chars().all(|c| c.is_ascii_uppercase()))
}

/// `SLIM --docs=off --recommends=off --locales=pt,en`.
pub(crate) fn parse_slim(rest: &str) -> std::result::Result<Slim, String> {
    let mut s = Slim::default();
    let fl = flags(rest, "SLIM")?;
    if fl.is_empty() {
        return Err("SLIM needs at least one of --docs=off, --recommends=off, --locales=…".into());
    }
    for (k, v) in fl {
        match k.as_str() {
            "docs" => s.docs = Some(on_off("SLIM", &k, &v)?),
            "recommends" => s.recommends = Some(on_off("SLIM", &k, &v)?),
            "locales" => {
                let codes: Vec<String> = v
                    .split(',')
                    .filter(|c| !c.is_empty())
                    .map(str::to_string)
                    .collect();
                if codes.is_empty() {
                    return Err("SLIM --locales needs at least one language (pt,en)".into());
                }
                if let Some(bad) = codes.iter().find(|c| !valid_locale(c)) {
                    return Err(format!(
                        "SLIM --locales: '{bad}' is not a language code (pt, en, pt_BR)"
                    ));
                }
                s.locales = Some(codes);
            }
            other => {
                return Err(format!(
                    "SLIM: unknown flag --{other} — supported: --docs --recommends --locales"
                ))
            }
        }
    }
    Ok(s)
}

/// `INSTALL <pkg>…`.
pub(crate) fn parse_install(rest: &str) -> std::result::Result<Vec<String>, String> {
    let pkgs: Vec<String> = rest.split_whitespace().map(str::to_string).collect();
    if pkgs.is_empty() {
        return Err("INSTALL needs at least one package".into());
    }
    if let Some(bad) = pkgs.iter().find(|p| !super::vmspec::valid_pkg(p)) {
        return Err(format!("INSTALL: '{bad}' is not a package name"));
    }
    Ok(pkgs)
}

/// `REMOVE --packages=a,b --paths=/x --services=u`. Flags may repeat.
pub(crate) fn parse_remove(rest: &str) -> std::result::Result<Removal, String> {
    let mut r = Removal::default();
    for (k, v) in flags(rest, "REMOVE")? {
        let items = v.split(',').filter(|i| !i.is_empty()).map(str::to_string);
        match k.as_str() {
            "packages" => {
                for p in items {
                    if !super::vmspec::valid_pkg(&p) {
                        return Err(format!("REMOVE --packages: '{p}' is not a package name"));
                    }
                    r.packages.push(p);
                }
            }
            "paths" => {
                for p in items {
                    super::vmspec::valid_remove_path(&p)
                        .map_err(|m| format!("REMOVE --paths '{p}': {m}"))?;
                    r.paths.push(p);
                }
            }
            "services" => {
                for u in items {
                    if !super::vmspec::valid_unit(&u) {
                        return Err(format!("REMOVE --services: '{u}' is not a unit name"));
                    }
                    r.services.push(u);
                }
            }
            other => {
                return Err(format!(
                    "REMOVE: unknown flag --{other} — supported: --packages --paths --services"
                ))
            }
        }
    }
    if r.packages.is_empty() && r.paths.is_empty() && r.services.is_empty() {
        return Err("REMOVE needs something to remove (--packages=, --paths=, --services=)".into());
    }
    Ok(r)
}

/// The unit types `AUTOSTART` takes. A `.target`/`.mount` is wiring, not
/// something that "starts", and enabling one by accident here is not what a
/// line naming services means.
const AUTOSTART_TYPES: &[&str] = &["service", "socket", "timer", "path"];

/// Every systemd unit type, to tell `foo.target` (a type we refuse) from
/// `foo.bar` (a service whose name has a dot).
const UNIT_TYPES: &[&str] = &[
    "service",
    "socket",
    "timer",
    "path",
    "target",
    "mount",
    "automount",
    "swap",
    "device",
    "scope",
    "slice",
];

/// `smbd` → `smbd.service`; `fstrim.timer` stays; `local-fs.target` is refused.
pub(crate) fn unit_name(u: &str) -> std::result::Result<String, String> {
    if !super::vmspec::valid_unit(u) {
        return Err(format!("'{u}' is not a unit name"));
    }
    match u.rsplit_once('.') {
        Some((stem, ty)) if UNIT_TYPES.contains(&ty) => {
            if stem.is_empty() {
                return Err(format!("'{u}' is not a unit name"));
            }
            if AUTOSTART_TYPES.contains(&ty) {
                Ok(u.to_string())
            } else {
                Err(format!(
                    "'{u}': AUTOSTART takes .service, .socket, .timer or .path units"
                ))
            }
        }
        _ => Ok(format!("{u}.service")),
    }
}

/// `AUTOSTART [--only] <unit>…`, accumulated into the stage's [`Autostart`].
pub(crate) fn parse_autostart(rest: &str, into: &mut Autostart) -> std::result::Result<(), String> {
    let mut named = 0;
    for w in rest.split_whitespace() {
        if let Some(flag) = w.strip_prefix("--") {
            match flag {
                "only" => into.only = true,
                other => {
                    return Err(format!(
                        "AUTOSTART: unknown flag --{other} — supported: --only"
                    ))
                }
            }
            continue;
        }
        let unit = unit_name(w).map_err(|m| format!("AUTOSTART {m}"))?;
        if !into.units.contains(&unit) {
            into.units.push(unit);
        }
        named += 1;
    }
    if named == 0 {
        return Err("AUTOSTART needs at least one unit".into());
    }
    Ok(())
}

/// `BUDGET 450M` → bytes. Binary units (`K`/`M`/`G` = KiB/MiB/GiB), the ones
/// `du` and `vm image ls` speak.
pub(crate) fn parse_budget(rest: &str) -> std::result::Result<u64, String> {
    let r = rest.trim();
    let (num, mult) = match r.chars().last() {
        Some('K') => (&r[..r.len() - 1], 1u64 << 10),
        Some('M') => (&r[..r.len() - 1], 1u64 << 20),
        Some('G') => (&r[..r.len() - 1], 1u64 << 30),
        _ => return Err(format!("BUDGET needs a size like 450M or 1G, got '{r}'")),
    };
    let n: u64 = num
        .parse()
        .map_err(|_| format!("BUDGET needs a size like 450M or 1G, got '{r}'"))?;
    if n == 0 {
        return Err("BUDGET must be greater than zero".into());
    }
    n.checked_mul(mult)
        .ok_or_else(|| format!("BUDGET '{r}' is too large"))
}

/// The cross-line rule of a stage: a unit cannot be both started and masked.
/// Masked wins at runtime, so the image would silently not run what the
/// recipe says it runs — exactly the contradiction worth refusing at parse.
pub(crate) fn check_stage(
    auto: &Autostart,
    removals: &[&Removal],
) -> std::result::Result<(), String> {
    for r in removals {
        for s in &r.services {
            let norm = unit_name(s).unwrap_or_else(|_| s.clone());
            if auto.units.contains(&norm) {
                return Err(format!(
                    "{norm} is in AUTOSTART and in REMOVE --services of the same stage — it cannot both start and be masked"
                ));
            }
        }
    }
    Ok(())
}

// ─── translation to guest commands ──────────────────────────────────────────

/// Runs `apt` on a dpkg guest and `dnf` on an rpm one, and FAILS on a guest
/// with neither — a step that silently did nothing is the failure this file
/// exists to prevent.
pub(crate) fn pm_switch(what: &str, apt: &str, dnf: &str) -> String {
    format!(
        "if command -v apt-get >/dev/null 2>&1; then {apt}; \
         elif command -v dnf >/dev/null 2>&1; then {dnf}; \
         else echo '{what}: the guest has neither apt-get nor dnf' >&2; exit 1; fi"
    )
}

/// The directories `--docs=off` keeps out.
const DOC_DIRS: &[&str] = &[
    "/usr/share/doc",
    "/usr/share/man",
    "/usr/share/info",
    "/usr/share/groff",
    "/usr/share/lintian",
];

/// `printf` of lines into a file, each line single-quoted (none of them carry
/// a `'`: they are built here from validated codes and constants).
fn write_lines(path: &str, lines: &[String]) -> String {
    let quoted: Vec<String> = lines.iter().map(|l| format!("'{l}'")).collect();
    format!("printf '%s\\n' {} > {path}", quoted.join(" "))
}

/// Sets `key=value` under `[main]` in `/etc/dnf/dnf.conf`, replacing it when
/// present. dnf has no drop-in directory for `[main]`, and the file shipped by
/// every cloud image holds only that section.
pub(crate) fn dnf_conf(key: &str, value: &str) -> String {
    format!(
        "if grep -q '^{key}=' /etc/dnf/dnf.conf; then sed -i 's/^{key}=.*/{key}={value}/' /etc/dnf/dnf.conf; \
         else printf '%s\\n' '{key}={value}' >> /etc/dnf/dnf.conf; fi"
    )
}

/// The `case` pattern that keeps a language's directory under
/// `/usr/share/locale`: `pt` keeps `pt`, `pt_BR`, `pt@…`, `pt.…`.
fn locale_case(codes: &[String]) -> String {
    codes
        .iter()
        .flat_map(|c| {
            [
                c.clone(),
                format!("{c}_*"),
                format!("{c}.*"),
                format!("{c}@*"),
            ]
        })
        .collect::<Vec<_>>()
        .join("|")
}

/// What `SLIM` becomes: the package manager is told first (so nothing
/// excluded comes in with a later `INSTALL`), then what the base image
/// already carried is removed once.
pub(crate) fn slim_ops(s: &Slim) -> Vec<CustomizeOp> {
    let mut ops = Vec::new();
    let docs_off = s.docs == Some(false);
    let recommends_off = s.recommends == Some(false);

    // ── tell the package manager ──────────────────────────────────────────
    let mut dpkg: Vec<String> = Vec::new();
    if docs_off {
        for d in DOC_DIRS {
            dpkg.push(format!("path-exclude={d}/*"));
        }
        // The licence text is what a redistributed image owes; it stays.
        dpkg.push("path-include=/usr/share/doc/*/copyright".into());
    }
    if let Some(codes) = &s.locales {
        dpkg.push("path-exclude=/usr/share/locale/*".into());
        dpkg.push("path-include=/usr/share/locale/locale.alias".into());
        for c in codes {
            for pat in [
                c.clone(),
                format!("{c}_*"),
                format!("{c}.*"),
                format!("{c}@*"),
            ] {
                dpkg.push(format!("path-include=/usr/share/locale/{pat}/*"));
            }
        }
    }
    let mut apt_parts: Vec<String> = Vec::new();
    let mut dnf_parts: Vec<String> = Vec::new();
    if !dpkg.is_empty() {
        apt_parts.push(format!(
            "mkdir -p /etc/dpkg/dpkg.cfg.d && {}",
            write_lines("/etc/dpkg/dpkg.cfg.d/90-delonix-slim", &dpkg)
        ));
    }
    if recommends_off {
        apt_parts.push(format!(
            "mkdir -p /etc/apt/apt.conf.d && {}",
            write_lines(
                "/etc/apt/apt.conf.d/90-delonix-slim",
                &[
                    "APT::Install-Recommends \"false\";".into(),
                    "APT::Install-Suggests \"false\";".into(),
                ],
            )
        ));
        dnf_parts.push(dnf_conf("install_weak_deps", "False"));
    }
    if docs_off {
        dnf_parts.push(dnf_conf("tsflags", "nodocs"));
    }
    if let Some(codes) = &s.locales {
        // rpm's own knob: only these languages' files are installed.
        dnf_parts.push(format!(
            "mkdir -p /etc/rpm && printf '%%_install_langs %s\\n' '{}' > /etc/rpm/macros.image-language-conf",
            codes.join(":")
        ));
    }
    if !apt_parts.is_empty() || !dnf_parts.is_empty() {
        let or_true = |v: Vec<String>| {
            if v.is_empty() {
                "true".to_string()
            } else {
                v.join(" && ")
            }
        };
        ops.push(CustomizeOp::RunCommand(pm_switch(
            "SLIM",
            &or_true(apt_parts),
            &or_true(dnf_parts),
        )));
    }

    // ── remove what the base already carried ──────────────────────────────
    if docs_off {
        ops.push(CustomizeOp::RunCommand(format!(
            "find /usr/share/doc -mindepth 1 ! -type d ! -name copyright -delete 2>/dev/null; \
             find /usr/share/doc -mindepth 1 -type d -empty -delete 2>/dev/null; \
             rm -rf {}; true",
            DOC_DIRS[1..]
                .iter()
                .map(|d| format!("{d}/*"))
                .collect::<Vec<_>>()
                .join(" ")
        )));
    }
    if let Some(codes) = &s.locales {
        ops.push(CustomizeOp::RunCommand(format!(
            "for d in /usr/share/locale/*/; do n=$(basename \"$d\"); \
             case \"$n\" in {}) ;; *) rm -rf -- \"$d\" ;; esac; done",
            locale_case(codes)
        )));
    }
    ops
}

/// `INSTALL` — through the package manager, so `SLIM`'s rules apply to it.
/// The index is refreshed on every `INSTALL`: an earlier one in the stage may
/// have been followed by a `RUN` that changed the sources.
pub(crate) fn install_op(pkgs: &[String]) -> CustomizeOp {
    let list = pkgs.join(" ");
    CustomizeOp::RunCommand(pm_switch(
        "INSTALL",
        &format!(
            "export DEBIAN_FRONTEND=noninteractive; apt-get update -q && apt-get install -y -q {list}"
        ),
        &format!("dnf install -y {list}"),
    ))
}

/// What `INSTALL` leaves behind and the image must not carry: ~180 MiB of
/// `.deb` already installed plus ~180 MiB of indexes on an apt guest, measured
/// on the golden recipe (see `vmimage::common_customization_steps`). Runs once
/// at the end of a stage that installed anything.
pub(crate) fn package_cache_op() -> CustomizeOp {
    CustomizeOp::RunCommand(pm_switch(
        "INSTALL",
        "apt-get clean && rm -rf /var/lib/apt/lists/*",
        "dnf clean all",
    ))
}

/// `REMOVE` — packages (with what only they needed), then units, then paths.
pub(crate) fn remove_ops(r: &Removal) -> Vec<CustomizeOp> {
    let mut ops = Vec::new();
    if !r.packages.is_empty() {
        let list = r.packages.join(" ");
        ops.push(CustomizeOp::RunCommand(pm_switch(
            "REMOVE",
            &format!(
                "export DEBIAN_FRONTEND=noninteractive; apt-get purge -y {list} && apt-get autoremove -y --purge"
            ),
            &format!("dnf remove -y {list}"),
        )));
    }
    for u in &r.services {
        ops.push(CustomizeOp::RunCommand(format!(
            "systemctl disable {u} 2>/dev/null || true; systemctl mask {u}"
        )));
    }
    for p in &r.paths {
        ops.push(CustomizeOp::RunCommand(format!("rm -rf -- '{p}'")));
    }
    ops
}

/// Units `AUTOSTART --only` never disables, whatever the recipe lists: the
/// ones the ENGINE needs to reach and drive the VM, and the ones that load the
/// guest's mandatory access control. Without the first the image boots into
/// something `vm create` cannot configure (cloud-init), connect to (network),
/// log into (console) or shut down cleanly (guest agent); without the second
/// it boots with its confinement silently off, which no line of a recipe that
/// lists services ever meant. Only the ones present in the guest matter; the
/// list covers both families.
pub(crate) const AUTOSTART_PLUMBING: &[&str] = &[
    // D-Bus: logind, resolved, networkd and the agent talk over it (on Fedora
    // `dbus-broker` is the enabled unit behind the `dbus` alias).
    "dbus.service",
    "dbus-broker.service",
    // Network, either stack.
    "systemd-networkd.service",
    "systemd-networkd.socket",
    "systemd-networkd-wait-online.service",
    "systemd-network-generator.service",
    "systemd-resolved.service",
    "networking.service",
    "NetworkManager.service",
    "NetworkManager-wait-online.service",
    "NetworkManager-dispatcher.service",
    // cloud-init, across the unit renames of 23.x/24.x.
    "cloud-init-local.service",
    "cloud-init.service",
    "cloud-init-main.service",
    "cloud-init-network.service",
    "cloud-config.service",
    "cloud-final.service",
    "cloud-init-hotplugd.socket",
    // Consoles: `vm console` is a serial getty.
    "getty@.service",
    "serial-getty@.service",
    // How the engine shuts down, freezes and queries a VM.
    "qemu-guest-agent.service",
    // SELinux guests relabel on the next boot through it when needed.
    "selinux-autorelabel-mark.service",
    // Loads the AppArmor profiles at boot (Debian/Ubuntu). Found on the first
    // real build (Debian trixie, 2026-11-27): `--only` without this entry
    // disabled it along with the apt timers.
    "apparmor.service",
];

/// Where `AUTOSTART --only` lists what it disabled, one unit per line, for
/// [`footprint_op`] to carry back to the host. `virt-customize` does not show
/// a command's stdout outside its debug mode, so an `echo` would never reach
/// the build log — measured on the same build.
pub(crate) const AUTOSTART_DISABLED_PATH: &str = "/usr/share/delonix/autostart-disabled.txt";

/// The keep-set of `--only`: the declared units, the template behind each
/// declared instance (`serial-getty@ttyS0.service` is enabled through
/// `serial-getty@.service`), and the plumbing.
fn autostart_keep(a: &Autostart) -> Vec<String> {
    let mut keep: Vec<String> = a.units.clone();
    for u in &a.units {
        if let Some((tmpl, rest)) = u.split_once('@') {
            if let Some((_, ty)) = rest.rsplit_once('.') {
                keep.push(format!("{tmpl}@.{ty}"));
            }
        }
    }
    keep.extend(AUTOSTART_PLUMBING.iter().map(|s| s.to_string()));
    keep.sort();
    keep.dedup();
    keep
}

/// `AUTOSTART` — applied at the END of its stage, after every `INSTALL`,
/// `RUN` and `REMOVE`: a package's postinst enables its own services, so only
/// then is the list of enabled units the final one.
///
/// Enabling is fail-closed (no `|| true`): a unit that does not exist is a
/// recipe that names something the image does not have. With `--only`, every
/// other enabled `.service/.socket/.timer/.path` is disabled — not masked, so
/// a dependency can still pull it in — and listed in
/// [`AUTOSTART_DISABLED_PATH`], which the build reads back and prints, so the
/// log says what the image no longer starts.
///
/// **What a kept unit asked for is kept too.** A unit enabled through
/// `WantedBy=`/`RequiredBy=` of a kept unit lives in that unit's `.wants/` or
/// `.requires/` directory, and disabling it breaks the unit the recipe asked
/// for. Found on the first real build (Debian trixie, 2026-11-27):
/// `sshd-keygen.service` is enabled into `ssh.service.wants/`, and `--only ssh`
/// was disabling it.
pub(crate) fn autostart_ops(a: &Autostart) -> Vec<CustomizeOp> {
    let mut ops = Vec::new();
    if !a.units.is_empty() {
        ops.push(CustomizeOp::RunCommand(format!(
            "systemctl enable {}",
            a.units.join(" ")
        )));
    }
    if a.only {
        let keep = autostart_keep(a).join(" ");
        let list = AUTOSTART_DISABLED_PATH;
        ops.push(CustomizeOp::RunCommand(format!(
            "mkdir -p /usr/share/delonix && : > {list} && keep=' {keep} '; \
             systemctl list-unit-files --state=enabled --no-legend --type=service,socket,timer,path \
             | while read -r u _; do \
               case \"$keep\" in *\" $u \"*) continue ;; esac; \
               dep=; for k in $keep; do \
                 if [ -e \"/etc/systemd/system/$k.wants/$u\" ] || [ -e \"/etc/systemd/system/$k.requires/$u\" ]; then dep=1; break; fi; \
               done; \
               [ -n \"$dep\" ] && continue; \
               systemctl disable \"$u\" || exit 1; \
               echo \"$u\" >> {list}; \
             done"
        )));
    }
    ops
}

/// The per-instance identity a reusable image must not carry: `/etc/machine-id`
/// emptied (systemd binds a fresh id over it at boot and commits it once `/etc`
/// is writable; D-Bus reads the same file) and no SSH host keys (cloud-init
/// generates each clone's own from the NoCloud seed `vm create` attaches). The
/// same two commands `vm.yaml`'s `cleanup:` runs.
///
/// **Empty, not `uninitialized`.** `uninitialized` would make the clone's
/// first boot a systemd "first boot" (machine-id(5)), which runs
/// `ConditionFirstBoot=` units such as `sshd-keygen` — but also presets every
/// unit, and Debian's preset policy enables almost everything, which would
/// undo `AUTOSTART --only` on the first boot of every VM. An empty file is not
/// a first boot: unique id, nothing re-enabled.
pub(crate) fn identity_reset_ops() -> Vec<CustomizeOp> {
    vec![
        CustomizeOp::RunCommand(
            "truncate -s 0 /etc/machine-id; mkdir -p /var/lib/dbus; ln -sf /etc/machine-id /var/lib/dbus/machine-id"
                .into(),
        ),
        CustomizeOp::RunCommand("rm -f /etc/ssh/ssh_host_*".into()),
    ]
}

// ─── measurement ────────────────────────────────────────────────────────────

/// Where the build leaves the measurement inside the image. It stays there:
/// a few KiB that answer "what is taking the space?" without the recipe.
pub(crate) const FOOTPRINT_PATH: &str = "/usr/share/delonix/footprint.tsv";

/// Measures the image from inside: the root filesystem's used space (`du -x`,
/// so `/boot`, `/proc` and friends on their own filesystems are not counted)
/// and the installed size of every package, as its package manager declares
/// it. The units differ (dpkg in KiB, rpm in bytes) and are tagged, not
/// converted in shell — the host does that where it is tested.
pub(crate) fn footprint_op() -> CustomizeOp {
    CustomizeOp::RunCommand(format!(
        r#"mkdir -p /usr/share/delonix && {{ printf 'root\t%s\n' "$(du -sxk / | cut -f1)"; if command -v dpkg-query >/dev/null 2>&1; then dpkg-query -W -f='dpkg\t${{Installed-Size}}\t${{binary:Package}}\n'; elif command -v rpm >/dev/null 2>&1; then rpm -qa --qf 'rpm\t%{{SIZE}}\t%{{NAME}}\n'; fi; if [ -f {AUTOSTART_DISABLED_PATH} ]; then while read -r u; do printf 'disabled\t%s\n' "$u"; done < {AUTOSTART_DISABLED_PATH}; fi; {kernel}; }} > {FOOTPRINT_PATH}"#,
        kernel = super::vmfile_kernel::footprint_lines(),
    ))
}

/// What [`footprint_op`] measured, in bytes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Footprint {
    /// Used space of the root filesystem.
    pub(crate) root: Option<u64>,
    /// `(installed bytes, package)`, largest first.
    pub(crate) packages: Vec<(u64, String)>,
    /// Units `AUTOSTART --only` disabled, in the order it found them.
    pub(crate) disabled: Vec<String>,
    /// The kernel the image boots (`uname -r` shape), as `KERNEL` recorded it
    /// or as installed — never the build appliance's.
    pub(crate) kernel: Option<String>,
    /// That kernel's initrd, in bytes.
    pub(crate) initrd: Option<u64>,
}

/// Reads the file [`footprint_op`] wrote. Lines it does not understand are
/// skipped: this is a report, and a malformed line must not hide the rest.
pub(crate) fn parse_footprint(text: &str) -> Footprint {
    let mut f = Footprint::default();
    for line in text.lines() {
        let cols: Vec<&str> = line.split('\t').collect();
        match cols.as_slice() {
            ["root", kib] => f.root = kib.trim().parse::<u64>().ok().map(|k| k * 1024),
            ["dpkg", kib, name] => {
                // A package with no Installed-Size field prints an empty column.
                if let Ok(k) = kib.trim().parse::<u64>() {
                    f.packages.push((k * 1024, name.to_string()));
                }
            }
            ["rpm", bytes, name] => {
                if let Ok(b) = bytes.trim().parse::<u64>() {
                    f.packages.push((b, name.to_string()));
                }
            }
            ["disabled", unit] if !unit.is_empty() => f.disabled.push(unit.to_string()),
            ["kernel", v] if !v.trim().is_empty() => f.kernel = Some(v.trim().to_string()),
            ["initrd", bytes] => f.initrd = bytes.trim().parse::<u64>().ok(),
            _ => {}
        }
    }
    f.packages
        .sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    f
}

/// How many packages the report and a budget failure name.
const TOP: usize = 15;

fn top_lines(f: &Footprint) -> String {
    f.packages
        .iter()
        .take(TOP)
        .map(|(b, n)| format!("  {:>10}  {n}", super::output::fmt_size(*b)))
        .collect::<Vec<_>>()
        .join("\n")
}

/// For the build log: the root size and the largest packages, and — when
/// `AUTOSTART --only` ran — what it disabled.
pub(crate) fn report(f: &Footprint) -> String {
    let root = f
        .root
        .map(super::output::fmt_size)
        .unwrap_or_else(|| "?".into());
    let largest: Vec<String> = f
        .packages
        .iter()
        .take(5)
        .map(|(b, n)| format!("{n} {}", super::output::fmt_size(*b)))
        .collect();
    let mut out = super::po::tf(
        "footprint: root filesystem {root}, {n} packages — largest: {largest}",
        &[
            ("root", &root),
            ("n", &f.packages.len().to_string()),
            ("largest", &largest.join(", ")),
        ],
    );
    if let Some(k) = &f.kernel {
        out.push('\n');
        out.push_str(&super::po::tf(
            "kernel {kernel}, initrd {initrd}",
            &[
                ("kernel", k),
                (
                    "initrd",
                    &f.initrd
                        .map(super::output::fmt_size)
                        .unwrap_or_else(|| "?".into()),
                ),
            ],
        ));
    }
    if !f.disabled.is_empty() {
        out.push('\n');
        out.push_str(&super::po::tf(
            "AUTOSTART --only disabled {n}: {units}",
            &[
                ("n", &f.disabled.len().to_string()),
                ("units", &f.disabled.join(", ")),
            ],
        ));
    }
    out
}

/// `BUDGET` enforced. Fail-closed both ways: over the budget is an error, and
/// so is a budget that could not be checked — an image whose size nobody could
/// read is not an image that met its budget.
pub(crate) fn check_budget(f: &Footprint, budget: u64) -> Result<()> {
    let root = f.root.ok_or_else(|| {
        Error::Invalid(
            super::po::t("BUDGET: the image's footprint could not be measured, so the budget cannot be checked")
                .to_string(),
        )
    })?;
    if root > budget {
        return Err(Error::Invalid(super::po::tf(
            "BUDGET {budget} exceeded: the root filesystem uses {root}. Largest packages:\n{top}",
            &[
                ("budget", &super::output::fmt_size(budget)),
                ("root", &super::output::fmt_size(root)),
                ("top", &top_lines(f)),
            ],
        )));
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Every guest command this module generates, for the `sh -n` check in
    /// `vmfile_kernel::tests::generated_shell_parses`.
    pub(crate) fn all_generated_commands() -> Vec<String> {
        let mut a = Autostart::default();
        parse_autostart("--only smbd serial-getty@ttyS0 fstrim.timer", &mut a).unwrap();
        let mut ops = slim_ops(&parse_slim("--docs=off --recommends=off --locales=pt,en").unwrap());
        ops.push(install_op(&["samba".into(), "openssh-server".into()]));
        ops.push(package_cache_op());
        ops.extend(remove_ops(
            &parse_remove("--packages=a,b --services=nmbd --paths=/opt/x").unwrap(),
        ));
        ops.extend(autostart_ops(&a));
        ops.extend(identity_reset_ops());
        ops.push(footprint_op());
        cmds(&ops)
    }

    fn cmds(ops: &[CustomizeOp]) -> Vec<String> {
        ops.iter()
            .map(|o| match o {
                CustomizeOp::RunCommand(c) => c.clone(),
                other => panic!("unexpected op {other:?}"),
            })
            .collect()
    }

    #[test]
    fn slim_flags_parse_and_refuse_what_they_do_not_know() {
        let s = parse_slim("--docs=off --recommends=off --locales=pt,en").unwrap();
        assert_eq!(s.docs, Some(false));
        assert_eq!(s.recommends, Some(false));
        assert_eq!(s.locales, Some(vec!["pt".into(), "en".into()]));

        assert!(
            parse_slim("").is_err(),
            "a SLIM that says nothing is a typo"
        );
        assert!(parse_slim("--docs=no").is_err(), "on/off only");
        assert!(parse_slim("--manpages=off").is_err(), "unknown flag");
        assert!(parse_slim("docs=off").is_err(), "not a flag");
        assert!(parse_slim("--locales=").is_err());
        for bad in ["PT", "pt-BR", "p", "pt_br", "pt;rm", "*"] {
            assert!(parse_slim(&format!("--locales={bad}")).is_err(), "{bad}");
        }
        assert!(parse_slim("--locales=pt_BR,ast").is_ok());
    }

    #[test]
    fn slim_tells_both_package_managers_before_removing_what_is_there() {
        let ops = slim_ops(&parse_slim("--docs=off --recommends=off --locales=pt").unwrap());
        let c = cmds(&ops);
        // First the package manager, so a later INSTALL is already slim.
        assert!(c[0].contains("path-exclude=/usr/share/doc/*"), "{}", c[0]);
        assert!(
            c[0].contains("path-include=/usr/share/doc/*/copyright"),
            "{}",
            c[0]
        );
        assert!(
            c[0].contains("path-exclude=/usr/share/locale/*"),
            "{}",
            c[0]
        );
        assert!(
            c[0].contains("path-include=/usr/share/locale/pt_*/*"),
            "{}",
            c[0]
        );
        assert!(
            c[0].contains("APT::Install-Recommends \"false\";"),
            "{}",
            c[0]
        );
        assert!(c[0].contains("install_weak_deps=False"), "{}", c[0]);
        assert!(c[0].contains("tsflags=nodocs"), "{}", c[0]);
        assert!(c[0].contains("%%_install_langs %s\\n' 'pt'"), "{}", c[0]);
        assert!(
            c[0].contains("neither apt-get nor dnf"),
            "fail-closed: {}",
            c[0]
        );
        // Then what the base already carried.
        assert!(c[1].contains("! -name copyright"), "{}", c[1]);
        assert!(c[1].contains("/usr/share/man/*"), "{}", c[1]);
        assert!(c[2].contains("pt|pt_*|pt.*|pt@*"), "{}", c[2]);
        assert_eq!(c.len(), 3);
    }

    #[test]
    fn slim_with_only_recommends_touches_no_files() {
        let c = cmds(&slim_ops(&parse_slim("--recommends=off").unwrap()));
        assert_eq!(c.len(), 1, "{c:?}");
        assert!(!c[0].contains("dpkg.cfg.d"), "{}", c[0]);
        assert!(cmds(&slim_ops(&parse_slim("--docs=on").unwrap())).is_empty());
    }

    #[test]
    fn install_and_remove_validate_names() {
        assert_eq!(
            parse_install("samba  smbclient").unwrap(),
            vec!["samba", "smbclient"]
        );
        assert!(parse_install("").is_err());
        assert!(parse_install("samba;reboot").is_err());

        let r = parse_remove(
            "--packages=a,b --services=nmbd --paths=/usr/share/samba/setup --packages=c",
        )
        .unwrap();
        assert_eq!(r.packages, vec!["a", "b", "c"]);
        assert_eq!(r.services, vec!["nmbd"]);
        assert_eq!(r.paths, vec!["/usr/share/samba/setup"]);
        assert!(parse_remove("").is_err());
        assert!(parse_remove("--paths=/etc").is_err(), "protected");
        assert!(parse_remove("--paths=relative").is_err());
        assert!(parse_remove("--users=bob").is_err(), "unknown flag");
        assert!(parse_remove("--services=-x").is_err());
    }

    #[test]
    fn remove_purges_with_autoremove_and_masks() {
        let c = cmds(&remove_ops(
            &parse_remove("--packages=snapd --services=nmbd --paths=/opt/x").unwrap(),
        ));
        assert!(c[0].contains("apt-get purge -y snapd && apt-get autoremove -y --purge"));
        assert!(c[0].contains("dnf remove -y snapd"));
        assert_eq!(
            c[1],
            "systemctl disable nmbd 2>/dev/null || true; systemctl mask nmbd"
        );
        assert_eq!(c[2], "rm -rf -- '/opt/x'");
    }

    #[test]
    fn unit_names_are_normalized_and_types_restricted() {
        assert_eq!(unit_name("smbd").unwrap(), "smbd.service");
        assert_eq!(unit_name("fstrim.timer").unwrap(), "fstrim.timer");
        assert_eq!(
            unit_name("serial-getty@ttyS0").unwrap(),
            "serial-getty@ttyS0.service"
        );
        assert_eq!(unit_name("foo.bar").unwrap(), "foo.bar.service");
        assert!(unit_name("local-fs.target").is_err());
        assert!(unit_name("-x").is_err());
        assert!(unit_name("a;b").is_err());
    }

    #[test]
    fn autostart_accumulates_and_only_keeps_the_plumbing() {
        let mut a = Autostart::default();
        parse_autostart("--only smbd ssh", &mut a).unwrap();
        parse_autostart("serial-getty@ttyS0 fstrim.timer smbd", &mut a).unwrap();
        assert!(a.only);
        assert_eq!(
            a.units,
            vec![
                "smbd.service",
                "ssh.service",
                "serial-getty@ttyS0.service",
                "fstrim.timer"
            ]
        );
        assert!(parse_autostart("--only", &mut Autostart::default()).is_err());
        assert!(parse_autostart("--all smbd", &mut Autostart::default()).is_err());

        let c = cmds(&autostart_ops(&a));
        assert_eq!(
            c[0],
            "systemctl enable smbd.service ssh.service serial-getty@ttyS0.service fstrim.timer"
        );
        assert!(!c[0].contains("||"), "enabling is fail-closed");
        // The template behind the declared instance, and the engine's plumbing.
        for k in [
            " serial-getty@.service ",
            " cloud-init-local.service ",
            " qemu-guest-agent.service ",
            " smbd.service ",
        ] {
            assert!(c[1].contains(k), "keep-set misses{k}: {}", c[1]);
        }
        assert!(c[1].contains("--state=enabled"));
        assert!(c[1].contains("systemctl disable \"$u\" || exit 1"));
        // What it disabled is written where the footprint carries it back:
        // virt-customize hides a command's stdout, so an echo is never seen.
        assert!(
            c[1].contains(&format!(": > {AUTOSTART_DISABLED_PATH}")),
            "{}",
            c[1]
        );
        assert!(
            c[1].contains(&format!("echo \"$u\" >> {AUTOSTART_DISABLED_PATH}")),
            "{}",
            c[1]
        );
        assert!(
            c[1].contains(" apparmor.service "),
            "the LSM loader is never disabled: {}",
            c[1]
        );
        // A unit a kept unit asked for (its `.wants/`/`.requires/`) is kept.
        assert!(
            c[1].contains("/etc/systemd/system/$k.wants/$u")
                && c[1].contains("/etc/systemd/system/$k.requires/$u"),
            "{}",
            c[1]
        );
    }

    #[test]
    fn autostart_without_only_disables_nothing() {
        let mut a = Autostart::default();
        parse_autostart("smbd", &mut a).unwrap();
        assert_eq!(cmds(&autostart_ops(&a)).len(), 1);
    }

    #[test]
    fn a_unit_cannot_both_autostart_and_be_masked() {
        let mut a = Autostart::default();
        parse_autostart("nmbd", &mut a).unwrap();
        let r = parse_remove("--services=nmbd.service").unwrap();
        assert!(check_stage(&a, &[&r]).is_err());
        let ok = parse_remove("--services=samba-ad-dc").unwrap();
        assert!(check_stage(&a, &[&ok]).is_ok());
    }

    #[test]
    fn budget_sizes() {
        assert_eq!(parse_budget("450M").unwrap(), 450 << 20);
        assert_eq!(parse_budget("1G").unwrap(), 1 << 30);
        assert_eq!(parse_budget("512K").unwrap(), 512 << 10);
        for bad in ["", "450", "450MB", "0M", "-1M", "x M", "99999999999999999G"] {
            assert!(parse_budget(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn footprint_reads_both_package_managers_and_skips_noise() {
        let f = parse_footprint(
            "root\t409600\n\
             dpkg\t2048\tlibc6:amd64\n\
             dpkg\t\tempty-size\n\
             rpm\t5242880\tsamba\n\
             disabled\tapt-daily.timer\n\
             kernel\t6.12.41+deb13-cloud-amd64\n\
             initrd\t18131120\n\
             disabled\t\n\
             garbage line\n",
        );
        assert_eq!(f.disabled, vec!["apt-daily.timer".to_string()]);
        assert_eq!(f.kernel.as_deref(), Some("6.12.41+deb13-cloud-amd64"));
        assert_eq!(f.initrd, Some(18_131_120));
        assert!(
            report(&f).contains("kernel 6.12.41+deb13-cloud-amd64, initrd 17.3 MiB"),
            "{}",
            report(&f)
        );
        assert!(report(&f).contains("AUTOSTART --only disabled 1: apt-daily.timer"));
        assert_eq!(f.root, Some(409_600 * 1024));
        assert_eq!(
            f.packages,
            vec![
                (5_242_880, "samba".to_string()),
                (2048 * 1024, "libc6:amd64".to_string())
            ]
        );
    }

    #[test]
    fn budget_fails_over_and_when_unmeasured() {
        let f = parse_footprint("root\t102400\ndpkg\t51200\tsamba\n");
        assert!(check_budget(&f, 200 << 20).is_ok());
        let e = check_budget(&f, 50 << 20).unwrap_err().to_string();
        assert!(
            e.contains("samba"),
            "the failure names the largest packages: {e}"
        );
        assert!(
            check_budget(&Footprint::default(), 1 << 30).is_err(),
            "unmeasured is not within budget"
        );
    }

    #[test]
    fn footprint_op_tags_units_and_writes_where_the_host_reads() {
        let c = cmds(&[footprint_op()]);
        assert!(c[0].contains("du -sxk /"), "{}", c[0]);
        assert!(
            c[0].contains("dpkg\\t${Installed-Size}\\t${binary:Package}"),
            "{}",
            c[0]
        );
        assert!(c[0].contains("rpm\\t%{SIZE}\\t%{NAME}"), "{}", c[0]);
        assert!(c[0].ends_with(&format!("> {FOOTPRINT_PATH}")), "{}", c[0]);
        assert!(
            c[0].contains(&format!("done < {AUTOSTART_DISABLED_PATH}")),
            "{}",
            c[0]
        );
    }
}
