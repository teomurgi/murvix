import base64
import contextlib
import importlib.util
import io
import os
from pathlib import Path
import re
import secrets
import shutil
import stat
import subprocess
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "signing_certificate", ROOT / "scripts/import-signing-certificate.py"
)
SIGNING = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SIGNING)


class SigningCertificateTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if shutil.which("openssl") is None:
            raise unittest.SkipTest("OpenSSL is required for real PKCS#12 fixtures")
        cls.fixture_directory = tempfile.TemporaryDirectory(prefix="murvix-signing-test-")
        cls.addClassCleanup(cls.fixture_directory.cleanup)
        cls.root = Path(cls.fixture_directory.name)
        cls.password = secrets.token_urlsafe(32)
        cls.environment = SIGNING.tool_environment()
        cls.environment["MURVIX_TEST_PKCS12_PASSWORD"] = cls.password
        # Actual signed leaf + issuer, rather than canned bytes or subprocess
        # mocks, exercise encryption, MAC verification, and chain preservation.
        old_umask = os.umask(0o077)
        try:
            cls.crypto([
                "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1",
                "-subj", "/CN=Murvix Test Issuer", "-keyout", str(cls.root / "issuer.key"),
                "-out", str(cls.root / "issuer.pem"),
            ])
            cls.crypto([
                "req", "-newkey", "rsa:2048", "-nodes", "-subj", "/CN=Murvix Test Signing",
                "-keyout", str(cls.root / "leaf.key"), "-out", str(cls.root / "leaf.csr"),
            ])
            cls.crypto([
                "x509", "-req", "-in", str(cls.root / "leaf.csr"),
                "-CA", str(cls.root / "issuer.pem"), "-CAkey", str(cls.root / "issuer.key"),
                "-set_serial", "2", "-days", "1", "-out", str(cls.root / "leaf.pem"),
            ])
            cls.crypto([
                "pkcs12", "-export", "-in", str(cls.root / "leaf.pem"),
                "-inkey", str(cls.root / "leaf.key"), "-certfile", str(cls.root / "issuer.pem"),
                "-out", str(cls.root / "modern.p12"),
                "-passout", "env:MURVIX_TEST_PKCS12_PASSWORD",
                "-keypbe", "AES-256-CBC", "-certpbe", "AES-256-CBC", "-macalg", "sha256",
            ])
        finally:
            os.umask(old_umask)
        cls.encoded = base64.b64encode((cls.root / "modern.p12").read_bytes()).decode("ascii")

    @classmethod
    def crypto(cls, args, environment=None):
        result = subprocess.run(
            ["openssl", *args], env=cls.environment if environment is None else environment,
            capture_output=True, check=False,
        )
        if result.returncode:
            # Neither command diagnostics nor arguments belong in failure output.
            raise AssertionError("OpenSSL fixture operation failed")
        return result

    def temporary(self):
        directory = tempfile.TemporaryDirectory(prefix="murvix-signing-case-", dir=self.root)
        self.addCleanup(directory.cleanup)
        return Path(directory.name)

    def test_modern_bundle_is_authenticated_and_normalized_without_changing_identity(self):
        info = self.crypto([
            "pkcs12", "-in", str(self.root / "modern.p12"), "-noout", "-info",
            "-passin", "env:MURVIX_TEST_PKCS12_PASSWORD",
        ]).stderr
        self.assertIn(b"PBES2", info)
        directory = self.temporary()
        normalized, password = SIGNING.prepare_certificate(self.encoded, self.password, directory)
        environment = self.environment.copy()
        environment["MURVIX_TEST_NORMALIZED_PASSWORD"] = password
        info = self.crypto([
            "pkcs12", "-in", str(normalized), "-noout", "-info",
            "-passin", "env:MURVIX_TEST_NORMALIZED_PASSWORD",
        ], environment).stderr.lower()
        self.assertIn(b"mac: sha1", info)
        self.assertEqual(info.count(b"pbewithsha1and3-keytripledes-cbc"), 2)
        self.assertNotIn(b"pbes2", info)
        self.assertTrue(password != self.password, "normalization must use a fresh password")
        extracted = directory / "roundtrip.pem"
        SIGNING.private_file(extracted)
        self.crypto([
            "pkcs12", "-in", str(normalized), "-out", str(extracted), "-nodes",
            "-passin", "env:MURVIX_TEST_NORMALIZED_PASSWORD",
        ], environment)
        original_public_key = self.crypto([
            "pkey", "-in", str(self.root / "leaf.key"), "-pubout",
        ]).stdout
        roundtrip_public_key = self.crypto(["pkey", "-in", str(extracted), "-pubout"]).stdout
        self.assertTrue(original_public_key == roundtrip_public_key, "private key changed")
        original_certificate = self.crypto([
            "x509", "-in", str(self.root / "leaf.pem"), "-outform", "DER",
        ]).stdout
        roundtrip_certificate = self.crypto([
            "x509", "-in", str(extracted), "-outform", "DER",
        ]).stdout
        self.assertTrue(original_certificate == roundtrip_certificate, "certificate changed")
        self.assertEqual(extracted.read_bytes().count(b"-----BEGIN CERTIFICATE-----"), 2)
        self.assertEqual(stat.S_IMODE(directory.stat().st_mode), 0o700)
        for path in directory.iterdir():
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o600)

    def test_wrong_password_does_not_retry_or_expose_diagnostics(self):
        directory = self.temporary()
        wrong_password = secrets.token_urlsafe(32)
        with self.assertRaises(SIGNING.SigningError) as error:
            SIGNING.prepare_certificate(self.encoded, wrong_password, directory)
        diagnostic = str(error.exception)
        self.assertTrue(wrong_password not in diagnostic, "password was exposed")
        self.assertIn("validation failed", diagnostic)
        self.assertNotIn("wrong password", diagnostic.lower())
        self.assertEqual((directory / "compatible.p12").stat().st_size, 0)

    def test_invalid_base64_is_rejected_before_any_command(self):
        directory = self.temporary()
        with self.assertRaisesRegex(SIGNING.SigningError, "not valid base64"):
            SIGNING.prepare_certificate("not%base64", self.password, directory)
        self.assertEqual(list(directory.iterdir()), [])

    def test_legacy_rc2_is_loaded_only_when_required_by_openssl(self):
        version = self.crypto(["version"]).stdout
        match = re.match(rb"OpenSSL (\d+)\.", version)
        if match is None or int(match[1]) < 3:
            self.skipTest("OpenSSL 3+ legacy provider is not available")
        directory = self.temporary()
        source = directory / "legacy.p12"
        SIGNING.private_file(source)
        self.crypto([
            "pkcs12", "-export", "-legacy", "-in", str(self.root / "leaf.pem"),
            "-inkey", str(self.root / "leaf.key"), "-out", str(source),
            "-passout", "env:MURVIX_TEST_PKCS12_PASSWORD",
            "-keypbe", "PBE-SHA1-3DES", "-certpbe", "PBE-SHA1-RC2-40", "-macalg", "sha1",
        ])
        encoded = base64.b64encode(source.read_bytes()).decode("ascii")
        with patch.object(SIGNING.subprocess, "run", wraps=subprocess.run) as run:
            normalized, password = SIGNING.prepare_certificate(encoded, self.password, self.temporary())
        extractions = [
            call.args[0] for call in run.call_args_list if "-nodes" in call.args[0]
        ]
        self.assertEqual(len(extractions), 2)
        self.assertNotIn("-legacy", extractions[0])
        self.assertIn("-legacy", extractions[1])
        environment = self.environment.copy()
        environment["MURVIX_TEST_NORMALIZED_PASSWORD"] = password
        self.crypto([
            "pkcs12", "-in", str(normalized), "-noout",
            "-passin", "env:MURVIX_TEST_NORMALIZED_PASSWORD",
        ], environment)

    def test_bundle_without_private_key_is_not_importable(self):
        source = self.temporary() / "certificates.p12"
        SIGNING.private_file(source)
        self.crypto([
            "pkcs12", "-export", "-nokeys", "-in", str(self.root / "leaf.pem"),
            "-out", str(source), "-passout", "env:MURVIX_TEST_PKCS12_PASSWORD",
        ])
        encoded = base64.b64encode(source.read_bytes()).decode("ascii")
        with self.assertRaisesRegex(SIGNING.SigningError, "matching private key and certificate"):
            SIGNING.prepare_certificate(encoded, self.password, self.temporary())

    def test_bundle_without_mac_is_rejected(self):
        source = self.temporary() / "unauthenticated.p12"
        SIGNING.private_file(source)
        self.crypto([
            "pkcs12", "-export", "-nomac", "-in", str(self.root / "leaf.pem"),
            "-inkey", str(self.root / "leaf.key"), "-out", str(source),
            "-passout", "env:MURVIX_TEST_PKCS12_PASSWORD",
        ])
        encoded = base64.b64encode(source.read_bytes()).decode("ascii")
        with self.assertRaisesRegex(SIGNING.SigningError, "no integrity MAC"):
            SIGNING.prepare_certificate(encoded, self.password, self.temporary())

    def test_native_import_errors_are_sanitized_after_real_crypto_validation(self):
        original_command = SIGNING.command
        security_commands = []
        keychain_password = secrets.token_urlsafe(32)

        def native_failure(args, environment):
            if args[0] != "security":
                return original_command(args, environment)
            security_commands.append(args)
            self.assertTrue("CERTIFICATE_P12" not in environment)
            self.assertTrue("CERTIFICATE_PASSWORD" not in environment)
            self.assertTrue("KEYCHAIN_PASSWORD" not in environment)
            if args[1] == "import":
                # Deliberately hostile diagnostics must not escape the boundary.
                return subprocess.CompletedProcess(args, 1, b"", self.password.encode())
            return subprocess.CompletedProcess(args, 0, b"", b"")

        with patch.object(SIGNING, "command", side_effect=native_failure):
            with self.assertRaises(SIGNING.SigningError) as error:
                SIGNING.import_certificate(
                    self.encoded, self.password, keychain_password, self.root / "test.keychain-db"
                )
        diagnostic = str(error.exception)
        self.assertIn("validated PKCS#12", diagnostic)
        self.assertNotIn("password", diagnostic.lower())
        self.assertTrue(self.password not in diagnostic, "password was exposed")
        imported = next(args for args in security_commands if args[1] == "import")
        self.assertFalse(Path(imported[2]).parent.exists(), "secret temporary files survived failure")

    def test_cli_validation_failure_cleans_up_without_keychain_operations(self):
        directory = self.temporary()
        environment = {
            "CERTIFICATE_P12": "not%base64", "CERTIFICATE_PASSWORD": self.password,
            "KEYCHAIN_PASSWORD": secrets.token_urlsafe(32),
        }
        stderr = io.StringIO()
        with patch.dict(os.environ, environment), patch.object(tempfile, "tempdir", str(directory)):
            with contextlib.redirect_stderr(stderr):
                self.assertEqual(SIGNING.main(["--keychain", str(directory / "test.keychain-db")]), 1)
        self.assertIn("not valid base64", stderr.getvalue())
        self.assertTrue(self.password not in stderr.getvalue(), "password was exposed")
        self.assertEqual(list(directory.iterdir()), [])


if __name__ == "__main__":
    unittest.main()
