//! `KERNEL` and `INITRD` — the kernel is declared in the Vmfile (ADR-0103, P1).
//!
//! `KERNEL <package> [--purge-others]` installs the kernel the image boots and,
//! with `--purge-others`, removes every other kernel with its modules.
//! `INITRD --modules=<list>` rebuilds that kernel's initrd with exactly those
//! drivers. `BOOT direct` (P2) and `KERNEL --file=` (which implies it) are not
//! here yet, and the parser says so rather than accepting them.
//!
//! **Never `uname -r`.** Inside `virt-customize` the running kernel is the
//! libguestfs appliance's, not the guest's: an initrd built for it would not
//! boot the VM, and a version read from it would be recorded as the image's.
//! The version always comes from the files the package owns, and is written to
//! [`KERNEL_VERSION_PATH`] — the same file the golden recipe already writes —
//! for `INITRD` and the measurement to read.

use super::vmimage::CustomizeOp;

/// Where the image records which kernel it boots. Shared with the golden
/// recipe (`vmimage::common_customization_steps` writes it too).
pub(crate) const KERNEL_VERSION_PATH: &str = "/etc/delonix-kernel-version";

/// `KERNEL <package> [--purge-others]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct KernelSpec {
    pub(crate) package: String,
    pub(crate) purge_others: bool,
}

/// `KERNEL linux-image-cloud-amd64 --purge-others`.
pub(crate) fn parse_kernel(rest: &str) -> std::result::Result<KernelSpec, String> {
    let mut package: Option<String> = None;
    let mut purge_others = false;
    for w in rest.split_whitespace() {
        match w {
            "--purge-others" => purge_others = true,
            f if f.starts_with("--file") => {
                return Err(
                    "KERNEL --file= arrives with BOOT direct (ADR-0103 phase P2): a kernel from \
                     outside the image is only bootable directly"
                        .into(),
                )
            }
            f if f.starts_with("--") => {
                return Err(format!(
                    "KERNEL: unknown flag {f} — supported: --purge-others"
                ))
            }
            p => {
                if package.is_some() {
                    return Err("KERNEL takes exactly one package".into());
                }
                if !super::vmspec::valid_pkg(p) {
                    return Err(format!("KERNEL: '{p}' is not a package name"));
                }
                package = Some(p.to_string());
            }
        }
    }
    let package = package.ok_or_else(|| {
        "KERNEL needs a package (linux-image-cloud-amd64, linux-image-virtual, kernel-core)"
            .to_string()
    })?;
    Ok(KernelSpec {
        package,
        purge_others,
    })
}

/// A kernel module name as `modprobe` takes it. Strict: it is written into a
/// config file and a dracut argument.
fn valid_module(m: &str) -> bool {
    !m.is_empty()
        && m.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && m.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

/// `INITRD --modules=virtio_pci,virtio_blk,ext4`.
pub(crate) fn parse_initrd(rest: &str) -> std::result::Result<Vec<String>, String> {
    let mut modules: Option<Vec<String>> = None;
    for w in rest.split_whitespace() {
        let list = w
            .strip_prefix("--modules=")
            .ok_or_else(|| format!("INITRD takes only --modules=<a,b,…>, got '{w}'"))?;
        if modules.is_some() {
            return Err("INITRD: --modules given twice — put the whole list in one".into());
        }
        let m: Vec<String> = list
            .split(',')
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
        if let Some(bad) = m.iter().find(|x| !valid_module(x)) {
            return Err(format!("INITRD --modules: '{bad}' is not a module name"));
        }
        modules = Some(m);
    }
    match modules {
        Some(m) if !m.is_empty() => Ok(m),
        _ => Err(
            "INITRD needs --modules=<list> — the drivers the VM needs to reach its root \
             (virtio_pci,virtio_blk and the root filesystem's module)"
                .into(),
        ),
    }
}

/// A shell function `kver` that prints the image's kernel version: the one
/// `KERNEL` recorded, when its kernel is still there, otherwise the highest
/// version installed (`/boot/vmlinuz-*` on Debian/Ubuntu,
/// `/lib/modules/*/vmlinuz` on Fedora/RHEL). Never `uname -r`.
pub(crate) fn kver_fn() -> String {
    let path = KERNEL_VERSION_PATH;
    format!(
        "kver() {{ v=; if [ -s {path} ]; then v=$(cat {path}); \
         if [ ! -e \"/boot/vmlinuz-$v\" ] && [ ! -e \"/lib/modules/$v/vmlinuz\" ]; then v=; fi; fi; \
         if [ -z \"$v\" ]; then v=$({{ ls /boot/vmlinuz-* 2>/dev/null | sed 's#.*/vmlinuz-##'; \
         ls -d /lib/modules/*/vmlinuz 2>/dev/null | sed 's#^/lib/modules/##; s#/vmlinuz$##'; }} | sort -uV | tail -1); fi; \
         printf '%s' \"$v\"; }}"
    )
}

/// What `KERNEL` becomes inside the guest. One command, so a failure at any
/// point fails the step:
///
/// 1. install the package (apt or dnf, under `SLIM`'s rules);
/// 2. resolve the version it brings from the files it owns — on apt, a
///    metapackage (`linux-image-cloud-amd64`) owns no kernel, so its
///    installed dependency `linux-image-<version>` is followed;
/// 3. with `--purge-others`, remove every other kernel image and module
///    package, then check the chosen kernel is still there;
/// 4. record the version in [`KERNEL_VERSION_PATH`].
pub(crate) fn kernel_op(k: &KernelSpec) -> CustomizeOp {
    let pkg = &k.package;
    let path = KERNEL_VERSION_PATH;
    let apt_purge = if k.purge_others {
        "others=$(dpkg-query -W -f='${binary:Package} ${db:Status-Abbrev}\\n' \
         'linux-image-[0-9]*' 'linux-image-unsigned-[0-9]*' 'linux-modules-[0-9]*' 'linux-modules-extra-[0-9]*' 2>/dev/null \
         | while read -r p s; do [ \"$s\" = ii ] || continue; case \"$p\" in *-\"$v\") ;; *) printf '%s\\n' \"$p\" ;; esac; done); \
         if [ -n \"$others\" ]; then apt-get purge -y $others && apt-get autoremove -y --purge || exit 1; fi; "
    } else {
        ""
    };
    let apt = format!(
        "export DEBIAN_FRONTEND=noninteractive; apt-get update -q && apt-get install -y -q {pkg} || exit 1; \
         case {pkg} in linux-image-[0-9]*) img={pkg} ;; \
         *) img=$(apt-cache depends --installed --recurse --no-recommends --no-suggests --no-conflicts \
              --no-breaks --no-replaces --no-enhances {pkg} 2>/dev/null | grep -o 'linux-image-[0-9][^ <>]*' | sort -uV | tail -1) ;; esac; \
         v=$(dpkg -L \"$img\" 2>/dev/null | sed -n 's#^/boot/vmlinuz-##p' | head -1); \
         if [ -z \"$v\" ]; then echo 'KERNEL: {pkg} installs no /boot/vmlinuz-<version>' >&2; exit 1; fi; \
         {apt_purge}"
    );
    let dnf_purge = if k.purge_others {
        "others=$(rpm -q --qf '%{NAME}-%{VERSION}-%{RELEASE}.%{ARCH}\\n' kernel kernel-core kernel-modules \
         kernel-modules-core kernel-modules-extra 2>/dev/null | while read -r p; do \
         case \"$p\" in *' '*|*'not installed'*) ;; *-\"$v\") ;; *) printf '%s\\n' \"$p\" ;; esac; done); \
         if [ -n \"$others\" ]; then dnf remove -y $others || exit 1; fi; "
    } else {
        ""
    };
    let dnf = format!(
        "dnf install -y {pkg} || exit 1; \
         v=$(rpm -ql {pkg} 2>/dev/null | sed -n 's#^/lib/modules/\\([^/]*\\)/vmlinuz$#\\1#p' | head -1); \
         [ -n \"$v\" ] || v=$(rpm -q --requires {pkg} 2>/dev/null | sed -n 's/^kernel-core-uname-r = //p' | head -1); \
         if [ -z \"$v\" ]; then echo 'KERNEL: cannot tell which kernel {pkg} installs' >&2; exit 1; fi; \
         {dnf_purge}"
    );
    let record = format!(
        "if [ ! -e \"/boot/vmlinuz-$v\" ] && [ ! -e \"/lib/modules/$v/vmlinuz\" ]; then \
         echo \"KERNEL: kernel $v is gone after the purge\" >&2; exit 1; fi; \
         printf '%s\\n' \"$v\" > {path}"
    );
    // `pm_switch` puts each branch between `then … ;` itself: a branch that
    // already ends in `;` would make `; ;`, a shell syntax error — found on
    // the first real build, and now caught by `generated_shell_parses`.
    let trim = |b: &str| b.trim_end().trim_end_matches(';').to_string();
    CustomizeOp::RunCommand(format!(
        "{}; {record}",
        super::vmfile_slim::pm_switch("KERNEL", &trim(&apt), &trim(&dnf))
    ))
}

/// What `INITRD` becomes inside the guest: the image's initrd rebuilt for the
/// image's kernel ([`kver_fn`]) with exactly `modules`, never host-only (the
/// host, inside `virt-customize`, is the appliance). The rule is also written
/// to the generator's configuration, so a later rebuild of the initrd keeps it.
///
/// - initramfs-tools (Debian/Ubuntu): `MODULES=list` + `/etc/initramfs-tools/modules`;
/// - dracut (Fedora/RHEL): `hostonly="no"` and `drivers+=` — dracut's exclusive
///   list, so the generic image's hundreds of drivers do not come along.
pub(crate) fn initrd_op(modules: &[String]) -> CustomizeOp {
    let lines = modules
        .iter()
        .map(|m| format!("'{m}'"))
        .collect::<Vec<_>>()
        .join(" ");
    let spaced = modules.join(" ");
    CustomizeOp::RunCommand(format!(
        "{kver}; v=$(kver); if [ -z \"$v\" ]; then echo 'INITRD: the image has no kernel' >&2; exit 1; fi; \
         if command -v update-initramfs >/dev/null 2>&1; then \
           mkdir -p /etc/initramfs-tools/conf.d && printf 'MODULES=list\\n' > /etc/initramfs-tools/conf.d/90-delonix.conf \
           && printf '%s\\n' {lines} > /etc/initramfs-tools/modules \
           && rm -f \"/boot/initrd.img-$v\" && update-initramfs -c -k \"$v\"; \
         elif command -v dracut >/dev/null 2>&1; then \
           mkdir -p /etc/dracut.conf.d && printf 'hostonly=\"no\"\\ndrivers+=\" %s \"\\n' '{spaced}' > /etc/dracut.conf.d/90-delonix.conf \
           && dracut --force --no-hostonly --drivers '{spaced}' \"/boot/initramfs-$v.img\" \"$v\"; \
         else echo 'INITRD: the guest has neither update-initramfs nor dracut' >&2; exit 1; fi",
        kver = kver_fn(),
    ))
}

/// The footprint lines for the kernel: `kernel\t<version>` and
/// `initrd\t<bytes>`, so every build records its kernel (not only those whose
/// base was a local image) and the initrd's size is in the build log.
pub(crate) fn footprint_lines() -> String {
    format!(
        "{kver}; v=$(kver); if [ -n \"$v\" ]; then printf 'kernel\\t%s\\n' \"$v\"; \
         for f in \"/boot/initrd.img-$v\" \"/boot/initramfs-$v.img\"; do \
         if [ -f \"$f\" ]; then printf 'initrd\\t%s\\n' \"$(stat -c %s \"$f\")\"; fi; done; fi",
        kver = kver_fn(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(op: CustomizeOp) -> String {
        match op {
            CustomizeOp::RunCommand(c) => c,
            other => panic!("unexpected op {other:?}"),
        }
    }

    #[test]
    fn kernel_parses_one_package_and_refuses_what_is_not_built() {
        assert_eq!(
            parse_kernel("linux-image-cloud-amd64 --purge-others").unwrap(),
            KernelSpec {
                package: "linux-image-cloud-amd64".into(),
                purge_others: true
            }
        );
        assert!(!parse_kernel("kernel-core").unwrap().purge_others);
        assert!(parse_kernel("").is_err());
        assert!(
            parse_kernel("--purge-others").is_err(),
            "a flag is not a package"
        );
        assert!(parse_kernel("a b").is_err(), "exactly one package");
        assert!(parse_kernel("k;reboot").is_err());
        assert!(parse_kernel("k --keep").is_err(), "unknown flag");
        let e = parse_kernel("--file=./vmlinux").unwrap_err();
        assert!(e.contains("P2"), "says when it arrives: {e}");
    }

    #[test]
    fn initrd_parses_a_module_list() {
        assert_eq!(
            parse_initrd("--modules=virtio_pci,virtio_blk,ext4").unwrap(),
            vec!["virtio_pci", "virtio_blk", "ext4"]
        );
        assert!(parse_initrd("").is_err());
        assert!(parse_initrd("--modules=").is_err());
        assert!(parse_initrd("virtio_blk").is_err(), "only --modules=");
        assert!(parse_initrd("--modules=a --modules=b").is_err());
        for bad in ["VIRTIO", "a;b", "-x", "a b", "*"] {
            assert!(parse_initrd(&format!("--modules={bad}")).is_err(), "{bad}");
        }
    }

    #[test]
    fn kernel_op_resolves_the_version_from_package_files_never_uname() {
        let c = cmd(kernel_op(
            &parse_kernel("linux-image-cloud-amd64 --purge-others").unwrap(),
        ));
        assert!(
            !c.contains("uname -r") && !c.contains("$(uname"),
            "the appliance's kernel is not the guest's: {c}"
        );
        assert!(c.contains("apt-get install -y -q linux-image-cloud-amd64"));
        // A metapackage is followed to the image package it depends on.
        assert!(c.contains("apt-cache depends --installed --recurse"), "{c}");
        assert!(c.contains("dpkg -L \"$img\""), "{c}");
        assert!(c.contains("rpm -ql linux-image-cloud-amd64"), "{c}");
        assert!(c.contains("kernel-core-uname-r = "), "{c}");
        // --purge-others removes the other kernels and checks the chosen one survived.
        assert!(
            c.contains("apt-get purge -y $others && apt-get autoremove -y --purge"),
            "{c}"
        );
        // Kept by an exact `-<version>` suffix, never a substring match.
        assert!(c.contains("*-\"$v\") ;;"), "{c}");
        assert!(!c.contains("grep -vF"), "{c}");
        assert!(c.contains("dnf remove -y $others"), "{c}");
        assert!(c.contains("is gone after the purge"), "{c}");
        assert!(c.ends_with(&format!("> {KERNEL_VERSION_PATH}")), "{c}");
        assert!(c.contains("neither apt-get nor dnf"), "fail-closed: {c}");

        let keep = cmd(kernel_op(&parse_kernel("kernel-core").unwrap()));
        assert!(!keep.contains("purge -y $others") && !keep.contains("remove -y $others"));
    }

    #[test]
    fn initrd_op_targets_the_image_kernel_and_is_never_host_only() {
        let c = cmd(initrd_op(&[
            "virtio_pci".into(),
            "virtio_blk".into(),
            "ext4".into(),
        ]));
        assert!(!c.contains("uname -r") && !c.contains("$(uname"), "{c}");
        assert!(c.contains("v=$(kver)"), "{c}");
        assert!(c.contains("MODULES=list"), "{c}");
        assert!(
            c.contains(
                "printf '%s\\n' 'virtio_pci' 'virtio_blk' 'ext4' > /etc/initramfs-tools/modules"
            ),
            "{c}"
        );
        assert!(c.contains("update-initramfs -c -k \"$v\""), "{c}");
        assert!(
            c.contains("dracut --force --no-hostonly --drivers 'virtio_pci virtio_blk ext4'"),
            "{c}"
        );
        assert!(c.contains("hostonly=\"no\""), "{c}");
        assert!(c.contains("neither update-initramfs nor dracut"), "{c}");
    }

    /// Every command these directives generate is valid shell. The string
    /// tests above check WHAT a command says; this checks the guest's `sh`
    /// can read it at all — the first real KERNEL build failed on a `; ;` no
    /// string assertion could see.
    #[test]
    fn generated_shell_parses() {
        let mut cmds = vec![
            cmd(kernel_op(
                &parse_kernel("linux-image-cloud-amd64 --purge-others").unwrap(),
            )),
            cmd(kernel_op(&parse_kernel("kernel-core").unwrap())),
            cmd(initrd_op(&["virtio_blk".into(), "ext4".into()])),
            format!("{}; true", footprint_lines()),
        ];
        cmds.extend(super::super::vmfile_slim::tests::all_generated_commands());
        for c in cmds {
            let out = std::process::Command::new("sh")
                .args(["-n", "-c", &c])
                .output()
                .expect("sh");
            assert!(
                out.status.success(),
                "not valid shell ({}):\n{c}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
    }

    /// The footprint's kernel lines must EXIT 0 whatever `/boot` holds: they
    /// end a `{ …; } > footprint.tsv` group, whose status is the build step's.
    /// The first real build failed because a `[ -f ] && …` loop ended on the
    /// initrd name the other generator uses. Run here for real — it only reads
    /// `/boot` and `/lib/modules` — so a host with a kernel exercises the loop,
    /// and one without exercises the empty case.
    #[test]
    fn footprint_lines_exit_zero_on_a_real_system() {
        let out = std::process::Command::new("sh")
            .args(["-c", &footprint_lines()])
            .output()
            .expect("sh");
        assert!(
            out.status.success(),
            "exit {:?}, stderr: {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        );
        let text = String::from_utf8_lossy(&out.stdout);
        for l in text.lines() {
            assert!(
                l.starts_with("kernel\t") || l.starts_with("initrd\t"),
                "{l}"
            );
        }
    }

    #[test]
    fn kver_prefers_the_recorded_kernel_only_while_it_exists() {
        let f = kver_fn();
        assert!(f.contains(&format!("cat {KERNEL_VERSION_PATH}")), "{f}");
        assert!(
            f.contains("/boot/vmlinuz-$v") && f.contains("/lib/modules/$v/vmlinuz"),
            "{f}"
        );
        assert!(f.contains("sort -uV | tail -1"), "{f}");
    }
}
