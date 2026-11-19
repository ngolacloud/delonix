"""Container launcher — the distroless replacement for docker/entrypoint.sh.

The runtime image is distroless (no /bin/sh), so the entrypoint cannot be a
shell script. This pure-Python launcher does what the old entrypoint did and is
invoked by the venv interpreter:

    ENTRYPOINT ["/usr/bin/python3.11", "/app/docker/launch.py"]
    CMD        ["/usr/bin/python3.11", "-m", "gunicorn.app.wsgiapp", "config.wsgi:application"]

Behaviour, kept identical to the former shell entrypoint:

- When MIGRATE_ON_START is truthy (true/1/yes), apply the migrations
  (`manage.py migrate --noinput`) and create a default admin user with a random
  password if it does not exist. MIGRATE_ON_START is for ONE replica (the
  SQLite default of the manifest); leave it unset with several replicas — each
  would race to migrate — and run `manage.py migrate` as a release step. A
  failed migration raises and stops the container (exit 1) before anything
  listens.
- Then `os.execv` the command in argv (gunicorn by default), replacing this
  process so it keeps the container's PID and receives SIGTERM directly — the
  same `exec "$@"` the shell entrypoint ended with. The event-consumer service
  overrides the command (`manage.py consume_events`) and reaches it the same
  way.
"""

from __future__ import annotations

import base64
import os
import sys

_TRUTHY = {"true", "1", "yes"}


def _ensure_admin() -> None:
    """Create the default superuser if MIGRATE_ON_START ran and it is missing."""
    import django

    django.setup()
    from django.contrib.auth.models import User

    admin_user = os.environ.get("ADMIN_USER", "admin")
    admin_email = os.environ.get("ADMIN_EMAIL", "admin@example.com")
    # A random password when none is given, like `openssl rand -base64 16`.
    admin_password = os.environ.get("ADMIN_PASSWORD") or base64.b64encode(os.urandom(16)).decode()

    if User.objects.filter(username=admin_user).exists():
        print(f"Admin user '{admin_user}' already exists")
        return
    User.objects.create_superuser(admin_user, admin_email, admin_password)
    port = os.environ.get("PORT", "__PORT__")
    banner = "=" * 60
    print(f"\n{banner}")
    print("Django Admin User Created")
    print(banner)
    print(f"URL:      http://localhost:{port}/admin/")
    print(f"Username: {admin_user}")
    print(f"Password: {admin_password}")
    print(f"{banner}\n")


def main() -> None:
    os.chdir("/app")
    os.environ.setdefault("DJANGO_SETTINGS_MODULE", "config.settings")

    if os.environ.get("MIGRATE_ON_START", "false").strip().lower() in _TRUTHY:
        from django.core.management import execute_from_command_line

        # A failed migration raises SystemExit/CommandError and stops the
        # container before gunicorn ever binds — the shell entrypoint's `set -e`.
        execute_from_command_line(["manage.py", "migrate", "--noinput"])
        _ensure_admin()

    # Replace this process with the command (CMD, or a compose `command:`
    # override). Defaults to gunicorn when nothing was passed.
    argv = sys.argv[1:] or ["/app/.venv/bin/gunicorn", "config.wsgi:application"]
    os.execv(argv[0], argv)


if __name__ == "__main__":
    main()
