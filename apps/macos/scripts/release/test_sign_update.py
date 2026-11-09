"""Exercise the actual signer and native verifier with disposable keys, never release secrets."""
import base64
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class SigningContractTests(unittest.TestCase):
    def test_native_and_tauri_key_envelopes_verify_in_the_native_updater(self):
        with tempfile.TemporaryDirectory(prefix="kudzu-signing-test-") as directory:
            work = Path(directory)
            secret, public = work / "fixture.key", work / "fixture.pub"
            subprocess.run(["minisign", "-G", "-W", "-s", str(secret), "-p", str(public)],
                           check=True, capture_output=True)
            archive = work / "Kudzu_0.7.0_universal.app.tar.gz"
            archive.write_bytes(b"Kudzu updater signing fixture\n")
            raw = secret.read_bytes()
            for value in (str(secret), raw.decode(), base64.b64encode(raw).decode()):
                environment = os.environ.copy()
                environment["TAURI_SIGNING_PRIVATE_KEY"] = value
                environment["TAURI_SIGNING_PRIVATE_KEY_PASSWORD"] = ""
                subprocess.run(["bash", str(ROOT / "scripts/release/sign-update.sh"), str(archive)],
                               env=environment, check=True, capture_output=True)
                signature = work / "fixture.minisig"
                signature.write_bytes(base64.b64decode(Path(str(archive) + ".sig").read_bytes()))
                subprocess.run(["minisign", "-V", "-m", str(archive), "-p", str(public), "-x", str(signature)],
                               check=True, capture_output=True)

            secret, public = work / "encrypted.key", work / "encrypted.pub"
            subprocess.run(["minisign", "-G", "-s", str(secret), "-p", str(public)],
                           input=b"fixture password\nfixture password\n", check=True,
                           capture_output=True, timeout=60)
            environment["TAURI_SIGNING_PRIVATE_KEY"] = base64.b64encode(secret.read_bytes()).decode()
            environment["TAURI_SIGNING_PRIVATE_KEY_PASSWORD"] = "fixture password"
            subprocess.run(["bash", str(ROOT / "scripts/release/sign-update.sh"), str(archive)],
                           env=environment, check=True, capture_output=True, timeout=60)

            # Compile the production verification implementation, rather than reimplementing
            # its crypto or trusting only minisign's verification of its own output.
            runner = work / "Verify.swift"
            runner.write_text('''import Foundation
@main enum Verify {
    static func main() throws {
        let args = CommandLine.arguments
        let bytes = try Data(contentsOf: URL(fileURLWithPath: args[1]))
        let publicKey = try Minisign.PublicKey(fileContents: String(contentsOfFile: args[2], encoding: .utf8))
        let signature = try Minisign.Signature(tauriBase64: String(contentsOfFile: args[1] + ".sig", encoding: .utf8))
        try Minisign.verify(bytes, signature: signature, publicKey: publicKey)
        guard signature.trustedComment.contains("file:Kudzu_0.7.0_universal.app.tar.gz") else { fatalError("filename missing") }
        do {
            try Minisign.verify(bytes + Data([0]), signature: signature, publicKey: publicKey)
            fatalError("tampered archive accepted")
        } catch Minisign.Failure.invalidSignature {}
        var changed = signature
        changed.trustedComment += "changed"
        do {
            try Minisign.verify(bytes, signature: changed, publicKey: publicKey)
            fatalError("tampered trusted comment accepted")
        } catch Minisign.Failure.invalidTrustedComment {}
    }
}
''')
            updater = ROOT / "Packages/KudzuKit/Sources/KudzuServices/Updater"
            executable = work / "verify"
            subprocess.run(["xcrun", "swiftc", "-O", str(updater / "Minisign.swift"),
                            str(updater / "Blake2b.swift"), str(runner), "-o", str(executable)],
                           check=True, capture_output=True)
            subprocess.run([str(executable), str(archive), str(public)], check=True, capture_output=True)


if __name__ == "__main__":
    unittest.main()
