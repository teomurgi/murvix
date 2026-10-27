#!/usr/bin/env python3
"""Validate and normalize a PKCS#12 identity before importing it into macOS."""
import argparse
import base64
import binascii
import os
from pathlib import Path
import re
import secrets
import subprocess
import sys
import tempfile


class SigningError(Exception):
    """A diagnostic that is safe to display without exposing signing secrets."""


def command(args, environment):
    # Never use check=True: CalledProcessError includes security's password args.
    # Tool diagnostics can contain sensitive material too, so capture both streams.
    try:
        return subprocess.run(args, env=environment, capture_output=True, check=False)
    except OSError:
        raise SigningError("A required signing tool could not be executed.") from None


def tool_environment():
    environment = os.environ.copy()
    for name in ("CERTIFICATE_P12", "CERTIFICATE_PASSWORD", "KEYCHAIN_PASSWORD"):
        environment.pop(name, None)
    environment["LC_ALL"] = "C"
    return environment


def private_file(path, content=b""):
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "wb") as stream:
        stream.write(content)


def needs_legacy_provider(diagnostics, environment):
    diagnostic = diagnostics.lower()
    # A failed MAC/password check is never a reason to retry with another provider.
    if any(message in diagnostic for message in (
        b"mac verify error", b"mac verify failure", b"invalid password",
    )):
        return False
    if b"unsupported" not in diagnostic or b"rc2" not in diagnostic:
        return False
    version = command(["openssl", "version"], environment)
    match = re.match(rb"OpenSSL (\d+)\.", version.stdout)
    return version.returncode == 0 and match is not None and int(match[1]) >= 3


def prepare_certificate(encoded, password, directory):
    """Authenticate and re-encrypt an identity, retaining its certificate chain.

    directory must be a private temporary directory. The plaintext PEM never
    leaves it; all files are mode 0600 and are removed by the caller's context.
    """
    try:
        bundle = base64.b64decode("".join(encoded.split()), validate=True)
    except (ValueError, binascii.Error):
        raise SigningError("CERTIFICATE_P12 is not valid base64.") from None
    if not bundle:
        raise SigningError("CERTIFICATE_P12 contains no certificate bundle.")
    source = directory / "input.p12"
    plaintext = directory / "identity.pem"
    normalized = directory / "compatible.p12"
    private_file(source, bundle)
    private_file(plaintext)
    private_file(normalized)

    output_password = secrets.token_urlsafe(32)
    environment = tool_environment()
    environment["MURVIX_PKCS12_INPUT_PASSWORD"] = password
    environment["MURVIX_PKCS12_OUTPUT_PASSWORD"] = output_password
    extract = [
        "openssl", "pkcs12", "-in", str(source), "-out", str(plaintext),
        "-passin", "env:MURVIX_PKCS12_INPUT_PASSWORD", "-nodes", "-info",
    ]
    result = command(extract, environment)
    if result.returncode and needs_legacy_provider(result.stderr, environment):
        # OpenSSL 3+ disables RC2 by default. Keep the same password and require
        # successful MAC verification even when loading that legacy provider.
        result = command([*extract, "-legacy"], environment)
    if result.returncode:
        raise SigningError(
            "PKCS#12 validation failed: check the certificate password, bundle "
            "integrity, and supported encryption algorithms."
        )
    if b"mac is absent" in result.stderr.lower():
        raise SigningError("PKCS#12 validation failed: the bundle has no integrity MAC.")

    # Explicit algorithms avoid OpenSSL's modern PBES2 defaults, which macOS's
    # native PKCS#12 importer may misreport as a MAC/password failure. Export
    # also checks that the certificate matches the private key.
    result = command([
        "openssl", "pkcs12", "-export", "-in", str(plaintext),
        "-out", str(normalized), "-passout", "env:MURVIX_PKCS12_OUTPUT_PASSWORD",
        "-keypbe", "PBE-SHA1-3DES", "-certpbe", "PBE-SHA1-3DES", "-macalg", "sha1",
    ], environment)
    if result.returncode:
        raise SigningError(
            "PKCS#12 normalization failed: a matching private key and certificate "
            "and supported export algorithms are required."
        )
    return normalized, output_password


def import_certificate(encoded, password, keychain_password, keychain):
    # TemporaryDirectory is mode 0700, including when the runner's umask is lax.
    # No secret files are written to the checkout or persist beyond this context.
    with tempfile.TemporaryDirectory(prefix="murvix-signing-") as temporary:
        directory = Path(temporary)
        certificate, import_password = prepare_certificate(encoded, password, directory)
        environment = tool_environment()
        steps = [
            (["security", "create-keychain", "-p", keychain_password, str(keychain)],
             "Could not create the signing keychain."),
            (["security", "set-keychain-settings", "-lut", "21600", str(keychain)],
             "Could not configure the signing keychain."),
            (["security", "unlock-keychain", "-p", keychain_password, str(keychain)],
             "Could not unlock the signing keychain."),
            (["security", "list-keychains", "-d", "user", "-s", str(keychain),
              str(Path.home() / "Library/Keychains/login.keychain-db")],
             "Could not configure the signing keychain search list."),
            # Native security requires password arguments. They are never logged,
            # nor included in exceptions, and the imported bundle uses a random
            # temporary password rather than the original certificate secret.
            (["security", "import", str(certificate), "-k", str(keychain),
              "-f", "pkcs12", "-P", import_password, "-T", "/usr/bin/codesign"],
             "macOS could not import the validated PKCS#12 identity; check keychain access and native import compatibility."),
            (["security", "set-key-partition-list", "-S", "apple-tool:,apple:,codesign:",
              "-s", "-k", keychain_password, str(keychain)],
             "Could not grant codesign access to the imported signing key."),
        ]
        for args, message in steps:
            if command(args, environment).returncode:
                raise SigningError(message)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--keychain", required=True, type=Path)
    args = parser.parse_args(argv)
    try:
        values = []
        for name in ("CERTIFICATE_P12", "CERTIFICATE_PASSWORD", "KEYCHAIN_PASSWORD"):
            value = os.environ.get(name)
            if not value:
                raise SigningError(f"{name} is required.")
            values.append(value)
        import_certificate(*values, args.keychain)
    except SigningError as error:
        print(f"Error: {error}", file=sys.stderr)
        return 1
    except (OSError, ValueError):
        # Filesystem/environment errors must not expose secret-bearing objects.
        print("Error: Signing certificate preparation could not be completed.", file=sys.stderr)
        return 1
    print("Validated signing identity imported and codesign access configured.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
