TenRate Windows 1.1.0.10 - UNSIGNED TEST CANDIDATE
==================================================
This is a preview of the Windows GUI from the integration source, not a formal update.
Its installer uses a separate per-user directory and shortcuts. It does
not replace the official TenRate launcher or Windows sync-service files,
configure Windows startup tasks, or deploy cloudflared.

IMPORTANT: The application may still use the same user data/account
directories as an existing TenRate installation. Back up your account
data before testing and avoid editing the same workspace concurrently
in a preview and the official client.

For a full production update, validate on a real Windows computer and
use the repository's release packager and integrity checks. The preview
does not claim background synchronization, identity or health acceptance.
It is not signed. Windows or SmartScreen may warn about an unknown
publisher. Do not disable operating-system security protections.

The ZIP is portable: extract it and launch timer_windows_client.exe.
The EXE setup installs the same test program in a separate directory.
Only install if you intentionally want to test Windows 1.1.0.10.
