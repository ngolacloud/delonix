//! OCI seccomp profiles — `--security-opt seccomp=<file.json>` and the CRI's
//! `SecurityProfile::Localhost`.
//!
//! The engine used to REFUSE a custom profile with a clear error, deliberately:
//! accepting one and silently running the built-in allowlist instead is the
//! failure mode this repo names as its worst. Refusing was the honest
//! placeholder; this module is the feature.
//!
//! Format: the runc/Docker/containerd JSON — `defaultAction` plus a list of
//! `{names, action}` rules. It is what a Kubernetes `localhostProfile` points
//! at, what every published hardening profile is written in, and what the CRI
//! conformance suite hands over.

use seccompiler::{
    SeccompAction, SeccompCmpArgLen, SeccompCmpOp, SeccompCondition, SeccompFilter, SeccompRule,
};
use std::collections::BTreeMap;

/// Syscall NAME to number, for THIS architecture — the calls every supported
/// architecture has.
///
/// Built from `libc::SYS_*` and not from a table of literals: the numbers
/// differ between x86_64 and aarch64, and a hardcoded table would be silently
/// wrong on one of them. A seccomp filter that denies the wrong syscall is far
/// worse than one that refuses to build.
///
/// Generated from the kernel's own `unistd` header, both halves of each pair
/// from the same name, so they cannot disagree.
///
/// The calls that exist on ONE architecture only live in [`SYSCALLS_ARCH`] —
/// putting them here made this crate fail to compile for aarch64 altogether
/// (60 × `E0425`, `libc::SYS_access` and friends), which took the whole Free
/// binary for arm64 with it. See the note there.
// These names are DELIBERATELY kept even though libc marks a handful of them
// deprecated (`create_module`, `get_kernel_syms`, `query_module` — dead since
// 2.6). A seccomp profile exists to DENY syscalls, and the ones nobody uses any
// more are exactly the ones a hardening profile names. Dropping them would make
// such a profile fail to load with "unknown syscall".
#[allow(deprecated)]
const SYSCALLS: &[(&str, i64)] = &[
    ("accept", libc::SYS_accept),
    ("accept4", libc::SYS_accept4),
    ("acct", libc::SYS_acct),
    ("add_key", libc::SYS_add_key),
    ("adjtimex", libc::SYS_adjtimex),
    ("bind", libc::SYS_bind),
    ("bpf", libc::SYS_bpf),
    ("brk", libc::SYS_brk),
    ("capget", libc::SYS_capget),
    ("capset", libc::SYS_capset),
    ("chdir", libc::SYS_chdir),
    ("chroot", libc::SYS_chroot),
    ("clock_adjtime", libc::SYS_clock_adjtime),
    ("clock_getres", libc::SYS_clock_getres),
    ("clock_gettime", libc::SYS_clock_gettime),
    ("clock_nanosleep", libc::SYS_clock_nanosleep),
    ("clock_settime", libc::SYS_clock_settime),
    ("clone", libc::SYS_clone),
    ("clone3", libc::SYS_clone3),
    ("close", libc::SYS_close),
    ("close_range", libc::SYS_close_range),
    ("connect", libc::SYS_connect),
    ("copy_file_range", libc::SYS_copy_file_range),
    ("delete_module", libc::SYS_delete_module),
    ("dup", libc::SYS_dup),
    ("dup3", libc::SYS_dup3),
    ("epoll_create1", libc::SYS_epoll_create1),
    ("epoll_ctl", libc::SYS_epoll_ctl),
    ("epoll_pwait", libc::SYS_epoll_pwait),
    ("epoll_pwait2", libc::SYS_epoll_pwait2),
    ("eventfd2", libc::SYS_eventfd2),
    ("execve", libc::SYS_execve),
    ("execveat", libc::SYS_execveat),
    ("exit", libc::SYS_exit),
    ("exit_group", libc::SYS_exit_group),
    ("faccessat", libc::SYS_faccessat),
    ("faccessat2", libc::SYS_faccessat2),
    ("fallocate", libc::SYS_fallocate),
    ("fanotify_init", libc::SYS_fanotify_init),
    ("fanotify_mark", libc::SYS_fanotify_mark),
    ("fchdir", libc::SYS_fchdir),
    ("fchmod", libc::SYS_fchmod),
    ("fchmodat", libc::SYS_fchmodat),
    ("fchown", libc::SYS_fchown),
    ("fchownat", libc::SYS_fchownat),
    ("fcntl", libc::SYS_fcntl),
    ("fdatasync", libc::SYS_fdatasync),
    ("fgetxattr", libc::SYS_fgetxattr),
    ("finit_module", libc::SYS_finit_module),
    ("flistxattr", libc::SYS_flistxattr),
    ("flock", libc::SYS_flock),
    ("fremovexattr", libc::SYS_fremovexattr),
    ("fsconfig", libc::SYS_fsconfig),
    ("fsetxattr", libc::SYS_fsetxattr),
    ("fsmount", libc::SYS_fsmount),
    ("fsopen", libc::SYS_fsopen),
    ("fspick", libc::SYS_fspick),
    ("fstat", libc::SYS_fstat),
    ("fstatfs", libc::SYS_fstatfs),
    ("fsync", libc::SYS_fsync),
    ("ftruncate", libc::SYS_ftruncate),
    ("futex", libc::SYS_futex),
    ("futex_waitv", libc::SYS_futex_waitv),
    ("getcpu", libc::SYS_getcpu),
    ("getcwd", libc::SYS_getcwd),
    ("getdents64", libc::SYS_getdents64),
    ("getegid", libc::SYS_getegid),
    ("geteuid", libc::SYS_geteuid),
    ("getgid", libc::SYS_getgid),
    ("getgroups", libc::SYS_getgroups),
    ("getitimer", libc::SYS_getitimer),
    ("get_mempolicy", libc::SYS_get_mempolicy),
    ("getpeername", libc::SYS_getpeername),
    ("getpgid", libc::SYS_getpgid),
    ("getpid", libc::SYS_getpid),
    ("getppid", libc::SYS_getppid),
    ("getpriority", libc::SYS_getpriority),
    ("getrandom", libc::SYS_getrandom),
    ("getresgid", libc::SYS_getresgid),
    ("getresuid", libc::SYS_getresuid),
    ("getrlimit", libc::SYS_getrlimit),
    ("get_robust_list", libc::SYS_get_robust_list),
    ("getrusage", libc::SYS_getrusage),
    ("getsid", libc::SYS_getsid),
    ("getsockname", libc::SYS_getsockname),
    ("getsockopt", libc::SYS_getsockopt),
    ("gettid", libc::SYS_gettid),
    ("gettimeofday", libc::SYS_gettimeofday),
    ("getuid", libc::SYS_getuid),
    ("getxattr", libc::SYS_getxattr),
    ("init_module", libc::SYS_init_module),
    ("inotify_add_watch", libc::SYS_inotify_add_watch),
    ("inotify_init1", libc::SYS_inotify_init1),
    ("inotify_rm_watch", libc::SYS_inotify_rm_watch),
    ("io_cancel", libc::SYS_io_cancel),
    ("ioctl", libc::SYS_ioctl),
    ("io_destroy", libc::SYS_io_destroy),
    ("io_getevents", libc::SYS_io_getevents),
    ("ioprio_get", libc::SYS_ioprio_get),
    ("ioprio_set", libc::SYS_ioprio_set),
    ("io_setup", libc::SYS_io_setup),
    ("io_submit", libc::SYS_io_submit),
    ("io_uring_enter", libc::SYS_io_uring_enter),
    ("io_uring_register", libc::SYS_io_uring_register),
    ("io_uring_setup", libc::SYS_io_uring_setup),
    ("kcmp", libc::SYS_kcmp),
    ("kexec_load", libc::SYS_kexec_load),
    ("keyctl", libc::SYS_keyctl),
    ("kill", libc::SYS_kill),
    ("landlock_add_rule", libc::SYS_landlock_add_rule),
    ("landlock_create_ruleset", libc::SYS_landlock_create_ruleset),
    ("landlock_restrict_self", libc::SYS_landlock_restrict_self),
    ("lgetxattr", libc::SYS_lgetxattr),
    ("linkat", libc::SYS_linkat),
    ("listen", libc::SYS_listen),
    ("listxattr", libc::SYS_listxattr),
    ("llistxattr", libc::SYS_llistxattr),
    ("lookup_dcookie", libc::SYS_lookup_dcookie),
    ("lremovexattr", libc::SYS_lremovexattr),
    ("lseek", libc::SYS_lseek),
    ("lsetxattr", libc::SYS_lsetxattr),
    ("madvise", libc::SYS_madvise),
    ("mbind", libc::SYS_mbind),
    ("membarrier", libc::SYS_membarrier),
    ("memfd_create", libc::SYS_memfd_create),
    ("memfd_secret", libc::SYS_memfd_secret),
    ("migrate_pages", libc::SYS_migrate_pages),
    ("mincore", libc::SYS_mincore),
    ("mkdirat", libc::SYS_mkdirat),
    ("mknodat", libc::SYS_mknodat),
    ("mlock", libc::SYS_mlock),
    ("mlock2", libc::SYS_mlock2),
    ("mlockall", libc::SYS_mlockall),
    ("mmap", libc::SYS_mmap),
    ("mount", libc::SYS_mount),
    ("mount_setattr", libc::SYS_mount_setattr),
    ("move_mount", libc::SYS_move_mount),
    ("move_pages", libc::SYS_move_pages),
    ("mprotect", libc::SYS_mprotect),
    ("mq_getsetattr", libc::SYS_mq_getsetattr),
    ("mq_notify", libc::SYS_mq_notify),
    ("mq_open", libc::SYS_mq_open),
    ("mq_timedreceive", libc::SYS_mq_timedreceive),
    ("mq_timedsend", libc::SYS_mq_timedsend),
    ("mq_unlink", libc::SYS_mq_unlink),
    ("mremap", libc::SYS_mremap),
    ("msgctl", libc::SYS_msgctl),
    ("msgget", libc::SYS_msgget),
    ("msgrcv", libc::SYS_msgrcv),
    ("msgsnd", libc::SYS_msgsnd),
    ("msync", libc::SYS_msync),
    ("munlock", libc::SYS_munlock),
    ("munlockall", libc::SYS_munlockall),
    ("munmap", libc::SYS_munmap),
    ("name_to_handle_at", libc::SYS_name_to_handle_at),
    ("nanosleep", libc::SYS_nanosleep),
    ("newfstatat", libc::SYS_newfstatat),
    ("nfsservctl", libc::SYS_nfsservctl),
    ("openat", libc::SYS_openat),
    ("openat2", libc::SYS_openat2),
    ("open_by_handle_at", libc::SYS_open_by_handle_at),
    ("open_tree", libc::SYS_open_tree),
    ("perf_event_open", libc::SYS_perf_event_open),
    ("personality", libc::SYS_personality),
    ("pidfd_getfd", libc::SYS_pidfd_getfd),
    ("pidfd_open", libc::SYS_pidfd_open),
    ("pidfd_send_signal", libc::SYS_pidfd_send_signal),
    ("pipe2", libc::SYS_pipe2),
    ("pivot_root", libc::SYS_pivot_root),
    ("pkey_alloc", libc::SYS_pkey_alloc),
    ("pkey_free", libc::SYS_pkey_free),
    ("pkey_mprotect", libc::SYS_pkey_mprotect),
    ("ppoll", libc::SYS_ppoll),
    ("prctl", libc::SYS_prctl),
    ("pread64", libc::SYS_pread64),
    ("preadv", libc::SYS_preadv),
    ("preadv2", libc::SYS_preadv2),
    ("prlimit64", libc::SYS_prlimit64),
    ("process_madvise", libc::SYS_process_madvise),
    ("process_mrelease", libc::SYS_process_mrelease),
    ("process_vm_readv", libc::SYS_process_vm_readv),
    ("process_vm_writev", libc::SYS_process_vm_writev),
    ("pselect6", libc::SYS_pselect6),
    ("ptrace", libc::SYS_ptrace),
    ("pwrite64", libc::SYS_pwrite64),
    ("pwritev", libc::SYS_pwritev),
    ("pwritev2", libc::SYS_pwritev2),
    ("quotactl", libc::SYS_quotactl),
    ("quotactl_fd", libc::SYS_quotactl_fd),
    ("read", libc::SYS_read),
    ("readahead", libc::SYS_readahead),
    ("readlinkat", libc::SYS_readlinkat),
    ("readv", libc::SYS_readv),
    ("reboot", libc::SYS_reboot),
    ("recvfrom", libc::SYS_recvfrom),
    ("recvmmsg", libc::SYS_recvmmsg),
    ("recvmsg", libc::SYS_recvmsg),
    ("remap_file_pages", libc::SYS_remap_file_pages),
    ("removexattr", libc::SYS_removexattr),
    ("renameat", libc::SYS_renameat),
    ("renameat2", libc::SYS_renameat2),
    ("request_key", libc::SYS_request_key),
    ("restart_syscall", libc::SYS_restart_syscall),
    ("rseq", libc::SYS_rseq),
    ("rt_sigaction", libc::SYS_rt_sigaction),
    ("rt_sigpending", libc::SYS_rt_sigpending),
    ("rt_sigprocmask", libc::SYS_rt_sigprocmask),
    ("rt_sigqueueinfo", libc::SYS_rt_sigqueueinfo),
    ("rt_sigreturn", libc::SYS_rt_sigreturn),
    ("rt_sigsuspend", libc::SYS_rt_sigsuspend),
    ("rt_sigtimedwait", libc::SYS_rt_sigtimedwait),
    ("rt_tgsigqueueinfo", libc::SYS_rt_tgsigqueueinfo),
    ("sched_getaffinity", libc::SYS_sched_getaffinity),
    ("sched_getattr", libc::SYS_sched_getattr),
    ("sched_getparam", libc::SYS_sched_getparam),
    ("sched_get_priority_max", libc::SYS_sched_get_priority_max),
    ("sched_get_priority_min", libc::SYS_sched_get_priority_min),
    ("sched_getscheduler", libc::SYS_sched_getscheduler),
    ("sched_rr_get_interval", libc::SYS_sched_rr_get_interval),
    ("sched_setaffinity", libc::SYS_sched_setaffinity),
    ("sched_setattr", libc::SYS_sched_setattr),
    ("sched_setparam", libc::SYS_sched_setparam),
    ("sched_setscheduler", libc::SYS_sched_setscheduler),
    ("sched_yield", libc::SYS_sched_yield),
    ("seccomp", libc::SYS_seccomp),
    ("semctl", libc::SYS_semctl),
    ("semget", libc::SYS_semget),
    ("semop", libc::SYS_semop),
    ("semtimedop", libc::SYS_semtimedop),
    ("sendmmsg", libc::SYS_sendmmsg),
    ("sendmsg", libc::SYS_sendmsg),
    ("sendto", libc::SYS_sendto),
    ("setdomainname", libc::SYS_setdomainname),
    ("setfsgid", libc::SYS_setfsgid),
    ("setfsuid", libc::SYS_setfsuid),
    ("setgid", libc::SYS_setgid),
    ("setgroups", libc::SYS_setgroups),
    ("sethostname", libc::SYS_sethostname),
    ("setitimer", libc::SYS_setitimer),
    ("set_mempolicy", libc::SYS_set_mempolicy),
    ("set_mempolicy_home_node", libc::SYS_set_mempolicy_home_node),
    ("setns", libc::SYS_setns),
    ("setpgid", libc::SYS_setpgid),
    ("setpriority", libc::SYS_setpriority),
    ("setregid", libc::SYS_setregid),
    ("setresgid", libc::SYS_setresgid),
    ("setresuid", libc::SYS_setresuid),
    ("setreuid", libc::SYS_setreuid),
    ("setrlimit", libc::SYS_setrlimit),
    ("set_robust_list", libc::SYS_set_robust_list),
    ("setsid", libc::SYS_setsid),
    ("setsockopt", libc::SYS_setsockopt),
    ("set_tid_address", libc::SYS_set_tid_address),
    ("settimeofday", libc::SYS_settimeofday),
    ("setuid", libc::SYS_setuid),
    ("setxattr", libc::SYS_setxattr),
    ("shmat", libc::SYS_shmat),
    ("shmctl", libc::SYS_shmctl),
    ("shmdt", libc::SYS_shmdt),
    ("shmget", libc::SYS_shmget),
    ("shutdown", libc::SYS_shutdown),
    ("sigaltstack", libc::SYS_sigaltstack),
    ("signalfd4", libc::SYS_signalfd4),
    ("socket", libc::SYS_socket),
    ("socketpair", libc::SYS_socketpair),
    ("splice", libc::SYS_splice),
    ("statfs", libc::SYS_statfs),
    ("statx", libc::SYS_statx),
    ("swapoff", libc::SYS_swapoff),
    ("swapon", libc::SYS_swapon),
    ("symlinkat", libc::SYS_symlinkat),
    ("sync", libc::SYS_sync),
    ("sync_file_range", libc::SYS_sync_file_range),
    ("syncfs", libc::SYS_syncfs),
    ("sysinfo", libc::SYS_sysinfo),
    ("syslog", libc::SYS_syslog),
    ("tee", libc::SYS_tee),
    ("tgkill", libc::SYS_tgkill),
    ("timer_create", libc::SYS_timer_create),
    ("timer_delete", libc::SYS_timer_delete),
    ("timerfd_create", libc::SYS_timerfd_create),
    ("timerfd_gettime", libc::SYS_timerfd_gettime),
    ("timerfd_settime", libc::SYS_timerfd_settime),
    ("timer_getoverrun", libc::SYS_timer_getoverrun),
    ("timer_gettime", libc::SYS_timer_gettime),
    ("timer_settime", libc::SYS_timer_settime),
    ("times", libc::SYS_times),
    ("tkill", libc::SYS_tkill),
    ("truncate", libc::SYS_truncate),
    ("umask", libc::SYS_umask),
    ("umount2", libc::SYS_umount2),
    ("uname", libc::SYS_uname),
    ("unlinkat", libc::SYS_unlinkat),
    ("unshare", libc::SYS_unshare),
    ("userfaultfd", libc::SYS_userfaultfd),
    ("utimensat", libc::SYS_utimensat),
    ("vhangup", libc::SYS_vhangup),
    ("vmsplice", libc::SYS_vmsplice),
    ("wait4", libc::SYS_wait4),
    ("waitid", libc::SYS_waitid),
    ("write", libc::SYS_write),
    ("writev", libc::SYS_writev),
];

/// The calls this architecture has and the others do not.
///
/// arm64 was designed after these were already legacy, so it never got them:
/// the kernel offers `openat` and not `open`, `newfstatat` and not `stat`,
/// `pselect6` and not `select`, `clone` and not `fork`. Sixty of them, plus the
/// x86-only oddities (`arch_prctl`, `iopl`, `modify_ldt`, `_sysctl`).
///
/// They are still named here because a hardening profile written for x86 names
/// them, and [`syscall_number`] answering `None` is exactly the right answer on
/// an architecture that has no such call — the caller already treats `None` as
/// "this kernel/arch has no such call" and skips the rule. What is NOT
/// acceptable is what happened before: referencing them unconditionally, so the
/// crate did not build at all off x86_64.
///
/// Adding a name here that arm64 DOES have is harmless (it just moves where it
/// is found); adding one to [`SYSCALLS`] that arm64 does NOT have breaks the
/// build for arm64. The gate that catches it is the `aarch64` leg of
/// `release-binaries` — which only helps when the CI actually runs.
#[cfg(target_arch = "x86_64")]
#[allow(deprecated)]
const SYSCALLS_ARCH: &[(&str, i64)] = &[
    ("access", libc::SYS_access),
    ("afs_syscall", libc::SYS_afs_syscall),
    ("alarm", libc::SYS_alarm),
    ("arch_prctl", libc::SYS_arch_prctl),
    ("chmod", libc::SYS_chmod),
    ("chown", libc::SYS_chown),
    ("creat", libc::SYS_creat),
    // create_module (174), get_kernel_syms (177) and query_module (178) were
    // removed from the kernel (Linux 5.18) and `libc` 0.2.191 stopped exposing
    // the constants. The x86_64 syscall number is stable ABI and never reused,
    // so we keep the denylist entry for defense-in-depth (old kernels) with the
    // literal instead of the constant.
    ("create_module", 174),
    ("dup2", libc::SYS_dup2),
    ("epoll_create", libc::SYS_epoll_create),
    ("epoll_ctl_old", libc::SYS_epoll_ctl_old),
    ("epoll_wait", libc::SYS_epoll_wait),
    ("epoll_wait_old", libc::SYS_epoll_wait_old),
    ("eventfd", libc::SYS_eventfd),
    ("fadvise64", libc::SYS_fadvise64),
    ("fchmodat2", libc::SYS_fchmodat2),
    ("fork", libc::SYS_fork),
    ("futimesat", libc::SYS_futimesat),
    ("getdents", libc::SYS_getdents),
    ("get_kernel_syms", 177), // see note on create_module (constant removed from libc 0.2.191)
    ("getpgrp", libc::SYS_getpgrp),
    ("getpmsg", libc::SYS_getpmsg),
    ("get_thread_area", libc::SYS_get_thread_area),
    ("inotify_init", libc::SYS_inotify_init),
    ("ioperm", libc::SYS_ioperm),
    ("iopl", libc::SYS_iopl),
    ("kexec_file_load", libc::SYS_kexec_file_load),
    ("lchown", libc::SYS_lchown),
    ("link", libc::SYS_link),
    ("lstat", libc::SYS_lstat),
    ("mkdir", libc::SYS_mkdir),
    ("mknod", libc::SYS_mknod),
    ("modify_ldt", libc::SYS_modify_ldt),
    ("open", libc::SYS_open),
    ("pause", libc::SYS_pause),
    ("pipe", libc::SYS_pipe),
    ("poll", libc::SYS_poll),
    ("putpmsg", libc::SYS_putpmsg),
    ("query_module", 178), // see note on create_module (constant removed from libc 0.2.191)
    ("readlink", libc::SYS_readlink),
    ("rename", libc::SYS_rename),
    ("rmdir", libc::SYS_rmdir),
    ("security", libc::SYS_security),
    ("select", libc::SYS_select),
    ("sendfile", libc::SYS_sendfile),
    ("set_thread_area", libc::SYS_set_thread_area),
    ("signalfd", libc::SYS_signalfd),
    ("stat", libc::SYS_stat),
    ("symlink", libc::SYS_symlink),
    ("_sysctl", libc::SYS__sysctl),
    ("sysfs", libc::SYS_sysfs),
    ("time", libc::SYS_time),
    ("tuxcall", libc::SYS_tuxcall),
    ("unlink", libc::SYS_unlink),
    ("uselib", libc::SYS_uselib),
    ("ustat", libc::SYS_ustat),
    ("utime", libc::SYS_utime),
    ("utimes", libc::SYS_utimes),
    ("vfork", libc::SYS_vfork),
    ("vserver", libc::SYS_vserver),
];

/// Every other architecture: nothing extra beyond the common table.
#[cfg(not(target_arch = "x86_64"))]
const SYSCALLS_ARCH: &[(&str, i64)] = &[];

/// Resolves a syscall name. `None` = this kernel/arch has no such call.
///
/// Linear scan over the two tables (~360 entries), once per profile rule at
/// container start.
/// A map would be faster and is not worth the code: this runs once, and the
/// numbers are already in the binary.
pub fn syscall_number(name: &str) -> Option<i64> {
    SYSCALLS
        .iter()
        .chain(SYSCALLS_ARCH.iter())
        .find(|(n, _)| *n == name)
        .map(|(_, nr)| *nr)
}

/// The reverse of [`syscall_number`]: a syscall NUMBER back to its name. Used to
/// render this engine's own numeric allowlist as an OCI seccomp profile (names)
/// for an exported bundle (`image export`), so the `runc`/`crun` that runs it
/// applies the SAME policy this engine's own init enforces. `None` = a number
/// with no name in either table.
pub fn syscall_name(nr: i64) -> Option<&'static str> {
    SYSCALLS
        .iter()
        .chain(SYSCALLS_ARCH.iter())
        .find(|(_, n)| *n == nr)
        .map(|(name, _)| *name)
}

/// A parsed OCI seccomp profile, reduced to what this engine can enforce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub default_action: Act,
    /// One entry per `(syscall, action)` with its argument conditions — syscall
    /// names are already resolved to this architecture's numbers, so nothing
    /// downstream has to know about names.
    pub rules: Vec<Rule>,
}

/// A single resolved syscall rule: when `nr` is invoked and every condition in
/// `args` holds, the kernel applies `act`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub nr: i64,
    pub act: Act,
    /// Argument conditions, **AND-ed** together (libseccomp/runc semantics for
    /// the args inside one rule). Empty means the rule matches the syscall
    /// unconditionally, whatever its arguments.
    pub args: Vec<ArgCond>,
}

/// One `(argument, comparison)` test, mirroring an OCI `linux.seccomp.syscalls[].args[]`
/// entry once resolved to a concrete operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArgCond {
    /// Which syscall argument, `0..=5`.
    pub index: u8,
    pub op: ArgOp,
}

/// The per-argument comparisons OCI allows. The `MaskedEq` form carries both the
/// mask and the datum because the OCI arg puts the mask in `value` and the datum
/// in `valueTwo`; the others compare the argument directly against `value`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgOp {
    Ne(u64),
    Lt(u64),
    Le(u64),
    Eq(u64),
    Ge(u64),
    Gt(u64),
    /// Matches when `(arg & mask) == datum` — `SCMP_CMP_MASKED_EQ`.
    MaskedEq { mask: u64, datum: u64 },
}

impl ArgCond {
    /// Lowers one condition to the seccompiler form.
    ///
    /// The width is always `Qword`: libseccomp (hence runc) compares the full
    /// 64-bit syscall argument, and the engine's own built-in allowlist already
    /// uses `Qword` — narrowing to `Dword` here would silently ignore the high
    /// 32 bits of a flag the operator meant to match.
    fn to_condition(self) -> Result<SeccompCondition, String> {
        let (op, value) = match self.op {
            ArgOp::Ne(v) => (SeccompCmpOp::Ne, v),
            ArgOp::Lt(v) => (SeccompCmpOp::Lt, v),
            ArgOp::Le(v) => (SeccompCmpOp::Le, v),
            ArgOp::Eq(v) => (SeccompCmpOp::Eq, v),
            ArgOp::Ge(v) => (SeccompCmpOp::Ge, v),
            ArgOp::Gt(v) => (SeccompCmpOp::Gt, v),
            ArgOp::MaskedEq { mask, datum } => (SeccompCmpOp::MaskedEq(mask), datum),
        };
        SeccompCondition::new(self.index, SeccompCmpArgLen::Qword, op, value)
            .map_err(|e| format!("invalid seccomp argument condition: {e}"))
    }
}

/// The subset of `SCMP_ACT_*` this engine implements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    Allow,
    Errno(u32),
    KillThread,
    Log,
    Trap,
}

impl Act {
    fn to_seccompiler(self) -> SeccompAction {
        match self {
            Act::Allow => SeccompAction::Allow,
            Act::Errno(e) => SeccompAction::Errno(e),
            Act::KillThread => SeccompAction::KillThread,
            Act::Log => SeccompAction::Log,
            Act::Trap => SeccompAction::Trap,
        }
    }
}

/// Parses one `SCMP_ACT_*` string.
///
/// `SCMP_ACT_KILL` is mapped to KILL_THREAD, which is what libseccomp has
/// always meant by the bare name and what runc does. `SCMP_ACT_TRACE` and
/// `SCMP_ACT_NOTIFY` are REJECTED rather than downgraded: both hand the
/// decision to a supervising process this engine does not run, and silently
/// turning "ask the supervisor" into "allow" would be a hole in a policy the
/// caller believes is enforced.
fn parse_action(s: &str, errno_ret: Option<u32>) -> Result<Act, String> {
    Ok(match s {
        "SCMP_ACT_ALLOW" => Act::Allow,
        "SCMP_ACT_ERRNO" => Act::Errno(errno_ret.unwrap_or(libc::EPERM as u32)),
        "SCMP_ACT_KILL" | "SCMP_ACT_KILL_THREAD" => Act::KillThread,
        "SCMP_ACT_LOG" => Act::Log,
        "SCMP_ACT_TRAP" => Act::Trap,
        "SCMP_ACT_KILL_PROCESS" => {
            // seccompiler has no KILL_PROCESS. KILL_THREAD on a container's pid
            // 1 ends the container anyway, and on any other thread it is
            // strictly more permissive — say so instead of pretending.
            return Err(
                "SCMP_ACT_KILL_PROCESS is not supported (use SCMP_ACT_KILL for kill-thread)".into(),
            );
        }
        other => return Err(format!("unsupported seccomp action '{other}'")),
    })
}

/// Parses an OCI seccomp profile from JSON.
///
/// **Fail-closed at every step.** An unknown action, an unknown syscall name, a
/// malformed document — all are errors that stop the container. The alternative
/// (skip what we do not understand) produces a filter that is a SUBSET of the
/// policy the operator wrote, which is exactly the silent weakening this whole
/// module exists to avoid.
///
/// The one deliberate exception is a name this ARCHITECTURE does not have: a
/// profile written for x86_64 naming `arch_prctl` is not wrong on aarch64, it
/// is inapplicable, and refusing it would make portable profiles impossible.
/// Those are counted and reported by the caller, never silently dropped.
/// Parses the `args` array of one syscall entry into AND-ed conditions.
///
/// Fail-closed like the rest of the parser: an unknown operator, a
/// non-integer field, or an argument index outside `0..=5` (a syscall has at
/// most six register arguments) is an error that stops the container, never a
/// skipped condition — a dropped condition is exactly the silent weakening of
/// the operator's policy this module exists to refuse.
fn parse_args(entry: &serde_json::Value) -> Result<Vec<ArgCond>, String> {
    let Some(arr) = entry.get("args").and_then(|a| a.as_array()) else {
        return Ok(Vec::new());
    };
    let mut conds = Vec::with_capacity(arr.len());
    for a in arr {
        let index = a
            .get("index")
            .and_then(|i| i.as_u64())
            .ok_or_else(|| "seccomp arg has no integer 'index'".to_string())?;
        if index > 5 {
            return Err(format!(
                "seccomp arg 'index' {index} is out of range (a syscall has arguments 0..=5)"
            ));
        }
        let index = index as u8;
        // `value` defaults to 0 (OCI treats it as optional); `valueTwo` only
        // matters for the masked form and likewise defaults to 0.
        let value = a.get("value").and_then(|v| v.as_u64()).unwrap_or(0);
        let value_two = a.get("valueTwo").and_then(|v| v.as_u64()).unwrap_or(0);
        let op = a
            .get("op")
            .and_then(|o| o.as_str())
            .ok_or_else(|| "seccomp arg has no 'op'".to_string())?;
        let op = match op {
            "SCMP_CMP_NE" => ArgOp::Ne(value),
            "SCMP_CMP_LT" => ArgOp::Lt(value),
            "SCMP_CMP_LE" => ArgOp::Le(value),
            "SCMP_CMP_EQ" => ArgOp::Eq(value),
            "SCMP_CMP_GE" => ArgOp::Ge(value),
            "SCMP_CMP_GT" => ArgOp::Gt(value),
            // The OCI arg carries the mask in `value` and the datum in `valueTwo`.
            "SCMP_CMP_MASKED_EQ" => ArgOp::MaskedEq {
                mask: value,
                datum: value_two,
            },
            other => return Err(format!("unsupported seccomp arg operator '{other}'")),
        };
        conds.push(ArgCond { index, op });
    }
    Ok(conds)
}

pub fn parse(json: &str) -> Result<(Profile, Vec<String>), String> {
    let v: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("invalid seccomp profile JSON: {e}"))?;
    let default_action = v
        .get("defaultAction")
        .and_then(|d| d.as_str())
        .ok_or_else(|| "seccomp profile has no 'defaultAction'".to_string())?;
    let default_action = parse_action(default_action, None)?;

    let mut rules = Vec::new();
    let mut unknown = Vec::new();
    if let Some(arr) = v.get("syscalls").and_then(|s| s.as_array()) {
        for entry in arr {
            let action = entry
                .get("action")
                .and_then(|a| a.as_str())
                .ok_or_else(|| "syscall entry has no 'action'".to_string())?;
            let errno_ret = entry
                .get("errnoRet")
                .and_then(|e| e.as_u64())
                .map(|e| e as u32);
            let act = parse_action(action, errno_ret)?;
            // `args` narrows a rule to particular argument values — the shape
            // the default Docker/containerd/runtime-tools profile uses to allow
            // `clone` only when it does NOT create a namespace, and to allow
            // just the safe `personality` values. These are translated, NOT
            // dropped: silently widening a rule that meant to block
            // `clone(CLONE_NEWUSER)` into "block clone" would break every
            // threaded program in the container, and silently dropping it would
            // open the hole the operator wrote the rule to close.
            let args = parse_args(entry)?;
            let names = entry
                .get("names")
                .and_then(|n| n.as_array())
                .ok_or_else(|| "syscall entry has no 'names'".to_string())?;
            for n in names {
                let n = n
                    .as_str()
                    .ok_or_else(|| "syscall name is not a string".to_string())?;
                match syscall_number(n) {
                    Some(nr) => rules.push(Rule {
                        nr,
                        act,
                        args: args.clone(),
                    }),
                    None => unknown.push(n.to_string()),
                }
            }
        }
    }
    Ok((
        Profile {
            default_action,
            rules,
        },
        unknown,
    ))
}

/// Compiles a parsed profile into a BPF program.
///
/// seccompiler models a filter as "match → one action, no match → another", so
/// a profile with more than TWO distinct actions cannot be expressed. That is
/// refused rather than approximated: the profiles that matter in practice
/// (`ALLOW` default with `ERRNO` exceptions, or the reverse) use exactly two.
pub fn compile(p: &Profile) -> Result<seccompiler::BpfProgram, String> {
    let arch = std::env::consts::ARCH
        .try_into()
        .map_err(|_| "architecture without seccomp support".to_string())?;
    // A rule whose action IS the default changes nothing, and seccompiler refuses
    // a filter whose match and mismatch actions are equal. Without dropping them,
    // two valid profiles were refused and the container aborted: an allow-all
    // profile with no rules (`{"defaultAction":"SCMP_ACT_ALLOW","syscalls":[]}`),
    // and one that restates the default for some syscalls.
    let effective: Vec<&Rule> = p
        .rules
        .iter()
        .filter(|r| r.act != p.default_action)
        .collect();
    let rule_actions: std::collections::BTreeSet<Act> = effective.iter().map(|r| r.act).collect();
    if rule_actions.len() > 1 {
        return Err(format!(
            "profile mixes {} different rule actions; only one non-default action is supported",
            rule_actions.len()
        ));
    }
    // With no effective rule nothing ever matches, so the match action only has
    // to differ from the default for the filter to be built.
    let match_action =
        rule_actions
            .into_iter()
            .next()
            .unwrap_or(if p.default_action == Act::Allow {
                Act::Errno(libc::EPERM as u32)
            } else {
                Act::Allow
            });
    // A syscall named with NO argument conditions matches unconditionally, so
    // such an entry SUPERSEDES any arg-filtered entry for the same syscall (it
    // already matches every call of it). Collect those first, then build only
    // the conditioned rules for the syscalls that lack an unconditional entry —
    // seccompiler ORs the per-syscall rule list, and an empty list is itself the
    // "match the whole syscall" case.
    let unconditional: std::collections::BTreeSet<i64> = effective
        .iter()
        .filter(|r| r.args.is_empty())
        .map(|r| r.nr)
        .collect();
    let mut map: BTreeMap<i64, Vec<SeccompRule>> = BTreeMap::new();
    for r in &effective {
        if unconditional.contains(&r.nr) {
            map.insert(r.nr, vec![]); // whole-syscall match
            continue;
        }
        let conds: Vec<SeccompCondition> = r
            .args
            .iter()
            .map(|c| c.to_condition())
            .collect::<Result<_, _>>()?;
        let rule = SeccompRule::new(conds).map_err(|e| format!("failed to build seccomp rule: {e}"))?;
        map.entry(r.nr).or_default().push(rule);
    }
    SeccompFilter::new(
        map,
        p.default_action.to_seccompiler(),
        match_action.to_seccompiler(),
        arch,
    )
    .and_then(|f| f.try_into())
    .map_err(|e| format!("failed to build the seccomp filter: {e}"))
}

impl PartialOrd for Act {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Act {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        format!("{self:?}").cmp(&format!("{other:?}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The profile shape the CRI conformance suite hands over.
    const BLOCK_HOSTNAME: &str = r#"{
        "defaultAction": "SCMP_ACT_ALLOW",
        "syscalls": [{"names": ["sethostname"], "action": "SCMP_ACT_ERRNO"}]
    }"#;

    #[test]
    fn parses_the_conformance_profile() {
        let (p, unknown) = parse(BLOCK_HOSTNAME).unwrap();
        assert_eq!(p.default_action, Act::Allow);
        assert_eq!(p.rules.len(), 1);
        assert_eq!(p.rules[0].nr, libc::SYS_sethostname);
        assert_eq!(p.rules[0].act, Act::Errno(libc::EPERM as u32));
        assert!(p.rules[0].args.is_empty());
        assert!(unknown.is_empty());
        assert!(compile(&p).is_ok());
    }

    #[test]
    fn names_resolve_to_this_architectures_number() {
        // Not literals: on aarch64 the numbers differ, and a fixed table would
        // be silently wrong on one of the two.
        assert_eq!(syscall_number("read"), Some(libc::SYS_read));
        assert_eq!(syscall_number("mount"), Some(libc::SYS_mount));
        assert_eq!(syscall_number("this_does_not_exist"), None);
    }

    /// The two tables must not name the same syscall twice.
    ///
    /// Splitting the table by architecture is a copy of 60 lines from one const
    /// to another, and the way that goes wrong is a name left behind in both.
    /// A duplicate is not fatal at run time (the first hit wins) but it means
    /// one of the two is unreachable and nobody notices.
    #[test]
    fn no_name_appears_in_both_tables() {
        for (name, _) in SYSCALLS_ARCH {
            assert!(
                !SYSCALLS.iter().any(|(n, _)| n == name),
                "{name} is in the common table AND in the arch one"
            );
        }
    }

    /// The common table may only name calls EVERY supported architecture has.
    ///
    /// This is the invariant that broke the build for arm64: `access`, `open`,
    /// `stat`, `fork`, `select` and 55 others do not exist there, and naming
    /// them in the common table made the crate fail to compile with 60 ×
    /// `E0425` — taking the Free binary for aarch64 with it. A unit test cannot
    /// prove the other architecture compiles (only a cross build does), so what
    /// it guards here is the shape: the arch-only table is where they live, and
    /// on x86_64 they still resolve.
    #[test]
    fn x86_only_calls_live_in_the_arch_table() {
        for name in [
            "access",
            "open",
            "stat",
            "fork",
            "select",
            "dup2",
            "arch_prctl",
        ] {
            #[cfg(target_arch = "x86_64")]
            {
                assert!(
                    SYSCALLS_ARCH.iter().any(|(n, _)| *n == name),
                    "{name} should be in the arch table"
                );
                assert!(
                    syscall_number(name).is_some(),
                    "{name} should resolve on x86_64"
                );
            }
            // Off x86_64 the right answer is `None` — the caller already reads
            // that as "this arch has no such call" and skips the rule.
            #[cfg(not(target_arch = "x86_64"))]
            assert_eq!(
                syscall_number(name),
                None,
                "{name} does not exist on this architecture"
            );
        }
    }

    #[test]
    fn fails_closed_on_what_it_does_not_understand() {
        // Unknown action: an error, not "skip the rule".
        assert!(parse(r#"{"defaultAction":"SCMP_ACT_BANANA"}"#).is_err());
        // With no defaultAction there is no policy to apply at all.
        assert!(parse(r#"{"syscalls":[]}"#).is_err());
        // Broken JSON.
        assert!(parse("{").is_err());
        // TRACE/NOTIFY hand the decision to a supervisor that does not exist
        // here: turning them into "allow" would be a hole in a policy the
        // operator believes is enforced.
        assert!(parse(
            r#"{"defaultAction":"SCMP_ACT_ALLOW","syscalls":[{"names":["read"],"action":"SCMP_ACT_NOTIFY"}]}"#
        )
        .is_err());
    }

    #[test]
    fn rules_with_args_are_translated_not_widened() {
        // A rule that only means to block `clone(CLONE_NEWUSER)` must stay
        // argument-filtered — widening it to "block clone" would break every
        // threaded program, and dropping it would open the hole it closes. It is
        // now translated faithfully into a masked-eq condition on arg 0.
        let j = r#"{"defaultAction":"SCMP_ACT_ALLOW","syscalls":[
            {"names":["clone"],"action":"SCMP_ACT_ERRNO",
             "args":[{"index":0,"value":268435456,"op":"SCMP_CMP_MASKED_EQ"}]}]}"#;
        let (p, unknown) = parse(j).unwrap();
        assert!(unknown.is_empty());
        assert_eq!(p.rules.len(), 1);
        assert_eq!(p.rules[0].nr, libc::SYS_clone);
        assert_eq!(
            p.rules[0].args,
            vec![ArgCond {
                index: 0,
                op: ArgOp::MaskedEq {
                    mask: 268435456, // CLONE_NEWUSER
                    datum: 0,
                },
            }]
        );
        assert!(compile(&p).is_ok());
    }

    #[test]
    fn every_oci_arg_operator_is_accepted() {
        // One rule per comparator, all sharing a single non-default action so the
        // two-action filter model still holds. Each must parse and lower cleanly.
        let j = r#"{"defaultAction":"SCMP_ACT_ALLOW","syscalls":[
            {"names":["read"],"action":"SCMP_ACT_ERRNO","args":[{"index":0,"value":1,"op":"SCMP_CMP_NE"}]},
            {"names":["write"],"action":"SCMP_ACT_ERRNO","args":[{"index":1,"value":2,"op":"SCMP_CMP_LT"}]},
            {"names":["openat"],"action":"SCMP_ACT_ERRNO","args":[{"index":2,"value":3,"op":"SCMP_CMP_LE"}]},
            {"names":["close"],"action":"SCMP_ACT_ERRNO","args":[{"index":0,"value":4,"op":"SCMP_CMP_EQ"}]},
            {"names":["dup3"],"action":"SCMP_ACT_ERRNO","args":[{"index":0,"value":5,"op":"SCMP_CMP_GE"}]},
            {"names":["fcntl"],"action":"SCMP_ACT_ERRNO","args":[{"index":1,"value":6,"op":"SCMP_CMP_GT"}]},
            {"names":["ioctl"],"action":"SCMP_ACT_ERRNO",
             "args":[{"index":1,"value":15,"valueTwo":1,"op":"SCMP_CMP_MASKED_EQ"}]}]}"#;
        let (p, unknown) = parse(j).unwrap();
        assert!(unknown.is_empty());
        assert_eq!(p.rules.len(), 7);
        assert!(compile(&p).is_ok());
    }

    #[test]
    fn two_args_on_one_rule_are_anded() {
        // Both conditions sit on the SAME rule, so the kernel AND-s them — this
        // is the libseccomp/runc meaning of several args in one entry.
        let j = r#"{"defaultAction":"SCMP_ACT_ALLOW","syscalls":[
            {"names":["socket"],"action":"SCMP_ACT_ERRNO",
             "args":[{"index":0,"value":2,"op":"SCMP_CMP_EQ"},
                     {"index":1,"value":1,"op":"SCMP_CMP_EQ"}]}]}"#;
        let (p, _) = parse(j).unwrap();
        assert_eq!(p.rules.len(), 1);
        assert_eq!(p.rules[0].args.len(), 2);
        assert!(compile(&p).is_ok());
    }

    #[test]
    fn the_runtime_tools_default_profile_shape_compiles() {
        // The exact shape `generate.New()` hands the suite: an ERRNO default, an
        // ALLOW list, `personality` pinned to safe values with SCMP_CMP_EQ, and
        // `clone` allowed only when it creates none of the listed namespaces via
        // SCMP_CMP_MASKED_EQ. This is the profile whose refusal the suite reported
        // as "linux.seccomp is outside the supported subset"; it must now build.
        let j = r#"{"defaultAction":"SCMP_ACT_ERRNO","syscalls":[
            {"names":["read","write","close","sethostname"],"action":"SCMP_ACT_ALLOW","args":[]},
            {"names":["personality"],"action":"SCMP_ACT_ALLOW","args":[{"index":0,"value":0,"op":"SCMP_CMP_EQ"}]},
            {"names":["personality"],"action":"SCMP_ACT_ALLOW","args":[{"index":0,"value":8,"op":"SCMP_CMP_EQ"}]},
            {"names":["personality"],"action":"SCMP_ACT_ALLOW","args":[{"index":0,"value":4294967295,"op":"SCMP_CMP_EQ"}]},
            {"names":["clone"],"action":"SCMP_ACT_ALLOW",
             "args":[{"index":0,"value":2114060288,"valueTwo":0,"op":"SCMP_CMP_MASKED_EQ"}]}]}"#;
        let (p, unknown) = parse(j).unwrap();
        assert!(unknown.is_empty());
        assert_eq!(p.default_action, Act::Errno(libc::EPERM as u32));
        // read/write/close/sethostname + 3×personality + clone = 8 resolved rules.
        assert_eq!(p.rules.len(), 8);
        // The one non-default action (ALLOW) keeps it inside the two-action model.
        assert!(compile(&p).is_ok());
    }

    #[test]
    fn an_unknown_arg_operator_fails_closed() {
        let j = r#"{"defaultAction":"SCMP_ACT_ALLOW","syscalls":[
            {"names":["read"],"action":"SCMP_ACT_ERRNO",
             "args":[{"index":0,"value":1,"op":"SCMP_CMP_BANANA"}]}]}"#;
        assert!(parse(j).is_err());
        // An argument index no syscall has is refused, not clamped.
        let j = r#"{"defaultAction":"SCMP_ACT_ALLOW","syscalls":[
            {"names":["read"],"action":"SCMP_ACT_ERRNO",
             "args":[{"index":9,"value":1,"op":"SCMP_CMP_EQ"}]}]}"#;
        assert!(parse(j).is_err());
    }

    #[test]
    fn an_unconditional_entry_supersedes_an_arg_filtered_one() {
        // If a syscall is allowed both unconditionally and under an arg filter,
        // the OR of the two is "allowed always" — the compiler must collapse to a
        // whole-syscall match, not emit a redundant narrower rule beside it.
        let j = r#"{"defaultAction":"SCMP_ACT_ERRNO","syscalls":[
            {"names":["clone"],"action":"SCMP_ACT_ALLOW","args":[]},
            {"names":["clone"],"action":"SCMP_ACT_ALLOW",
             "args":[{"index":0,"value":268435456,"op":"SCMP_CMP_MASKED_EQ"}]}]}"#;
        let (p, _) = parse(j).unwrap();
        assert!(compile(&p).is_ok());
    }

    #[test]
    fn name_inapplicable_on_this_architecture_is_reported_not_dropped() {
        // A portable profile names syscalls that only exist on one of the
        // architectures. That is inapplicable, not wrong — but whoever applies
        // it has to know.
        let j = r#"{"defaultAction":"SCMP_ACT_ALLOW","syscalls":[
            {"names":["read","a_syscall_from_another_arch"],"action":"SCMP_ACT_ERRNO"}]}"#;
        let (p, unknown) = parse(j).unwrap();
        assert_eq!(p.rules.len(), 1);
        assert_eq!(unknown, vec!["a_syscall_from_another_arch"]);
    }

    #[test]
    fn an_allow_all_profile_without_rules_compiles() {
        let (p, _) = parse(r#"{"defaultAction":"SCMP_ACT_ALLOW","syscalls":[]}"#).unwrap();
        assert!(compile(&p).is_ok());
        let (p, _) = parse(r#"{"defaultAction":"SCMP_ACT_ALLOW"}"#).unwrap();
        assert!(compile(&p).is_ok());
    }

    #[test]
    fn a_deny_all_profile_without_rules_compiles() {
        let (p, _) = parse(r#"{"defaultAction":"SCMP_ACT_ERRNO","syscalls":[]}"#).unwrap();
        assert!(compile(&p).is_ok());
    }

    #[test]
    fn rules_that_restate_the_default_are_not_a_second_action() {
        // Docker-style: an ERRNO default, ALLOW exceptions, and a rule that
        // repeats the default. Only the ALLOW exceptions change anything.
        let j = r#"{"defaultAction":"SCMP_ACT_ERRNO","syscalls":[
            {"names":["read","write"],"action":"SCMP_ACT_ALLOW"},
            {"names":["reboot"],"action":"SCMP_ACT_ERRNO"}]}"#;
        let (p, _) = parse(j).unwrap();
        assert!(compile(&p).is_ok());
        // An ALLOW default whose only rules also say ALLOW is allow-all.
        let j = r#"{"defaultAction":"SCMP_ACT_ALLOW","syscalls":[
            {"names":["read"],"action":"SCMP_ACT_ALLOW"}]}"#;
        let (p, _) = parse(j).unwrap();
        assert!(compile(&p).is_ok());
    }

    #[test]
    fn profile_with_two_distinct_actions_is_refused() {
        // seccompiler's model is "matched → one action, no match → another".
        // Approximating would apply a policy different from the one written.
        let j = r#"{"defaultAction":"SCMP_ACT_ALLOW","syscalls":[
            {"names":["read"],"action":"SCMP_ACT_ERRNO"},
            {"names":["write"],"action":"SCMP_ACT_KILL"}]}"#;
        let (p, _) = parse(j).unwrap();
        assert!(compile(&p).is_err());
    }
}
