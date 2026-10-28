"""Synthetic network faults through the real portable SDK preparation boundary."""
import contextlib
import hashlib
import http.client
import importlib.util
import io
import json
import pathlib
import ssl
import tarfile
import tempfile
import types
import unittest
import urllib.error
from unittest import mock


spec = importlib.util.spec_from_file_location("native_download_prepare", pathlib.Path(__file__).with_name("prepare-rust-portable.py"))
prepare = importlib.util.module_from_spec(spec)
spec.loader.exec_module(prepare)


class NativeDownloadGuards(unittest.TestCase):
    """No real network, SDK execution, device, model or user data is involved."""

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = pathlib.Path(self.temp.name)
        archive = io.BytesIO()
        with tarfile.open(fileobj=archive, mode="w:gz") as target:
            for name, data in {
                "contract.json": json.dumps({"version": "0.4.1", "header_hash": "f99ad4169374b8f3"}).encode(),
                "libtranscribe.dylib": b"synthetic, never executed",
            }.items():
                entry = tarfile.TarInfo("reviewed-sdk/" + name)
                entry.size = len(data)
                target.addfile(entry, io.BytesIO(data))
        self.payload = archive.getvalue()
        self.asset = {"file": "synthetic-reviewed-sdk.tar.gz", "sha256": hashlib.sha256(self.payload).hexdigest()}
        (self.root / "scripts").mkdir()
        (self.root / "scripts/native-assets.json").write_text(json.dumps({"osx-x64": self.asset}), encoding="utf-8")
        self.cache = self.root / ".build/native-download" / self.asset["file"]
        self.sleep = mock.Mock()
        self.patch_root = mock.patch.object(prepare, "ROOT", self.root)
        self.patch_root.start()
        self.addCleanup(self.patch_root.stop)
        # Also permits a retained pre-fix implementation to run this same trigger.
        patch_time = mock.patch.object(prepare, "time", types.SimpleNamespace(sleep=self.sleep), create=True)
        patch_time.start()
        self.addCleanup(patch_time.stop)

    def run_prepare(self):
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            prepare.prepare("osx-x64")

    def response(self, payload=None):
        return io.BytesIO(self.payload if payload is None else payload)

    def http_response(self, body, declared_length):
        """Exercise the stdlib's real bounded-read behavior, without a socket or network."""
        raw = b"HTTP/1.1 200 OK\r\nContent-Length: " + str(declared_length).encode() + b"\r\n\r\n" + body
        response = http.client.HTTPResponse(types.SimpleNamespace(makefile=lambda mode: io.BytesIO(raw)))
        response.begin()
        return response

    def test_timeout_then_success_retries_the_same_pinned_asset_without_partial_cache(self):
        """A transient connection failure must not permanently fail an otherwise valid build."""
        def opened(request, timeout):
            self.assertEqual(timeout, 120)
            self.assertEqual(request.full_url, "https://github.com/handy-computer/transcribe.cpp/releases/download/v0.4.1/" + self.asset["file"])
            self.assertFalse(self.cache.exists())
            if download.call_count == 1:
                raise urllib.error.URLError(TimeoutError("synthetic CI connection timeout"))
            return self.response()

        with mock.patch.object(prepare.urllib.request, "urlopen", side_effect=opened) as download:
            self.run_prepare()
        self.assertEqual(download.call_count, 2)
        self.sleep.assert_called_once_with(1)
        self.assertEqual(self.cache.read_bytes(), self.payload)
        self.assertEqual((self.root / ".build/rust-native-beta2-0.4.1-osx-x64/lib/libtranscribe.dylib").read_bytes(), b"synthetic, never executed")

    def test_interrupted_body_is_closed_and_never_cached_before_a_successful_retry(self):
        response = self.response()
        response.read = mock.Mock(side_effect=http.client.IncompleteRead(b"partial", 99))
        good = self.response()
        with mock.patch.object(prepare.urllib.request, "urlopen", side_effect=[response, good]) as download:
            self.run_prepare()
        self.assertEqual(download.call_count, 2)
        self.assertTrue(response.closed)
        self.assertTrue(good.closed)
        self.assertEqual(self.cache.read_bytes(), self.payload)

    def test_real_http_short_body_retries_before_checksum_validation(self):
        """Bounded HTTPResponse.read returns short bytes at EOF rather than raising."""
        short = self.http_response(b"partial", 100)
        complete = self.http_response(self.payload, len(self.payload))
        with mock.patch.object(prepare.urllib.request, "urlopen", side_effect=[short, complete]) as download:
            self.run_prepare()
        self.assertEqual(download.call_count, 2)
        self.assertTrue(short.closed)
        self.assertTrue(complete.closed)
        self.assertEqual(self.cache.read_bytes(), self.payload)

    def test_exhausted_transport_failure_is_bounded_and_preserves_existing_files(self):
        sentinel = self.root / "retained-proof.txt"
        sentinel.write_bytes(b"retain all earlier evidence")
        with mock.patch.object(prepare.urllib.request, "urlopen", side_effect=urllib.error.URLError(TimeoutError("synthetic"))) as download:
            with self.assertRaisesRegex(RuntimeError, "after 3 attempts"):
                self.run_prepare()
        self.assertEqual(download.call_count, 3)
        self.assertEqual(self.sleep.call_args_list, [mock.call(1), mock.call(2)])
        self.assertFalse(self.cache.exists())
        self.assertFalse((self.root / ".build/rust-native-beta2-0.4.1-osx-x64").exists())
        self.assertEqual(sentinel.read_bytes(), b"retain all earlier evidence")

    def test_transient_http_statuses_retry_but_keep_the_attempt_limit(self):
        failures = [urllib.error.HTTPError("https://example.invalid", code, "synthetic", {}, None) for code in [503, 429]]
        with mock.patch.object(prepare.urllib.request, "urlopen", side_effect=[*failures, self.response()]) as download:
            self.run_prepare()
        self.assertEqual(download.call_count, 3)
        self.assertEqual(self.sleep.call_args_list, [mock.call(1), mock.call(2)])

    def test_permanent_http_and_certificate_failures_are_not_retried(self):
        for failure in [urllib.error.HTTPError("https://example.invalid", 404, "synthetic", {}, None), urllib.error.URLError(ssl.SSLCertVerificationError("synthetic untrusted certificate"))]:
            with self.subTest(failure=type(failure).__name__), mock.patch.object(prepare.urllib.request, "urlopen", side_effect=failure) as download:
                with self.assertRaises(type(failure)):
                    self.run_prepare()
                self.assertEqual(download.call_count, 1)
                self.sleep.assert_not_called()
                self.assertFalse(self.cache.exists())

    def test_wrong_hash_is_not_retried_or_cached(self):
        with mock.patch.object(prepare.urllib.request, "urlopen", return_value=self.response(b"unexpected bytes")) as download:
            with self.assertRaisesRegex(RuntimeError, "fixed checksum"):
                self.run_prepare()
        download.assert_called_once()
        self.sleep.assert_not_called()
        self.assertFalse(self.cache.exists())

    def test_oversize_body_is_bounded_and_never_retried_or_cached(self):
        response = self.response(b"x" * 65)
        original_read = response.read
        response.read = mock.Mock(side_effect=original_read)
        with mock.patch.object(prepare, "MAX_ARCHIVE_BYTES", 64), mock.patch.object(prepare.urllib.request, "urlopen", return_value=response) as download:
            with self.assertRaisesRegex(RuntimeError, "limit"):
                self.run_prepare()
        response.read.assert_called_once_with(65)
        download.assert_called_once()
        self.sleep.assert_not_called()
        self.assertFalse(self.cache.exists())

    def test_verified_cache_is_reused_without_network_and_mismatched_cache_is_preserved(self):
        self.cache.parent.mkdir(parents=True)
        self.cache.write_bytes(self.payload)
        with mock.patch.object(prepare.urllib.request, "urlopen", side_effect=AssertionError("cached SDK must not download")) as download:
            self.run_prepare()
            self.cache.write_bytes(b"retain mismatched archive for inspection")
            with self.assertRaisesRegex(RuntimeError, "Cached native archive differs"):
                self.run_prepare()
        download.assert_not_called()
        self.assertEqual(self.cache.read_bytes(), b"retain mismatched archive for inspection")


if __name__ == "__main__":
    unittest.main()
