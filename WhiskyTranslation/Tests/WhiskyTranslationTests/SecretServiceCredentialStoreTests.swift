import Foundation
import Testing
@testable import WhiskyTranslation

#if os(Linux)
@Suite("Secret Service credentials")
struct SecretServiceCredentialStoreTests {
    @Test func storeUsesStableAttributesAndStdinOnly() async throws {
        let store = SecretServiceCredentialStore { arguments, input in
            #expect(arguments == ["store", "--label=Maltoria translation credential", "service", "com.daydalek.Maltoria.translation", "account", "fixture"])
            #expect(input == Data("private-value".utf8))
            return SecretToolResult(status: 0, output: Data(), diagnostic: Data())
        }
        #expect(await store.availability() == .persistentSecretService)
        try await store.store("private-value", for: "fixture")
    }

    @Test func lookupPreservesWhitespaceAndClearUsesSameAttributes() async throws {
        let store = SecretServiceCredentialStore { arguments, input in
            #expect(input == nil)
            #expect(Array(arguments.suffix(4)) == ["service", "com.daydalek.Maltoria.translation", "account", "fixture"])
            return SecretToolResult(status: 0, output: Data(" value\n\n".utf8), diagnostic: Data())
        }
        #expect(try await store.secret(for: "fixture") == " value\n")
        #expect(try await store.hasSecret(for: "fixture"))
        try await store.remove(reference: "fixture")
    }

    @Test func missingItemIsNotAnError() async throws {
        let store = SecretServiceCredentialStore { _, _ in
            SecretToolResult(status: 1, output: Data(), diagnostic: Data())
        }
        #expect(try await store.secret(for: "missing") == nil)
        #expect(try await !store.hasSecret(for: "missing"))
        try await store.remove(reference: "missing")
    }

    @Test func daemonFailureIsNotTreatedAsMissing() async {
        let store = SecretServiceCredentialStore { _, _ in
            SecretToolResult(status: 1, output: Data(), diagnostic: Data("service unavailable".utf8))
        }
        await #expect(throws: TranslationError.credentialOperationFailed(1)) { try await store.secret(for: "fixture") }
        await #expect(throws: TranslationError.credentialOperationFailed(1)) { try await store.store("value", for: "fixture") }
        await #expect(throws: TranslationError.credentialOperationFailed(1)) { try await store.remove(reference: "fixture") }
    }

    @Test func missingExecutableAndInvalidInputFailClearly() async {
        let store = SecretServiceCredentialStore { _, _ in throw CocoaError(.fileNoSuchFile) }
        await #expect(throws: TranslationError.credentialUnavailable) { try await store.secret(for: "fixture") }
        await #expect(throws: TranslationError.credentialUnavailable) { try await store.store("", for: "fixture") }
        await #expect(throws: TranslationError.credentialUnavailable) { try await store.store("value", for: "") }
    }

    @Test func availabilityReflectsExecutablePresence() async {
        let missing = SecretServiceCredentialStore(executableURL: URL(fileURLWithPath: "/nonexistent/secret-tool"))
        #expect(await missing.availability() == .unavailable)
        let available = SecretServiceCredentialStore(executableURL: URL(fileURLWithPath: "/bin/sh"))
        #expect(await available.availability() == .persistentSecretService)
        await #expect(throws: TranslationError.credentialUnavailable) { try await missing.secret(for: "fixture") }
    }

    @Test func drainsBothProcessPipesWithoutDeadlock() throws {
        let result = try SecretServiceCredentialStore.execute([
            "-c", "i=0; while [ $i -lt 8192 ]; do printf 'diagnostic-data-0123456789\\n' >&2; i=$((i+1)); done; printf 'value\\n'"
        ], nil, executableURL: URL(fileURLWithPath: "/bin/sh"))
        #expect(result.status == 0)
        #expect(result.output == Data("value\n".utf8))
        #expect(result.diagnostic.count > 131_072)
    }

    @Test(.enabled(if: ProcessInfo.processInfo.environment["WHISKY_TEST_SECRET_SERVICE"] == "1"))
    func realSecretServiceRoundTrip() async throws {
        let reference = "roundtrip-" + UUID().uuidString
        let secret = UUID().uuidString
        try await SecretServiceCredentialStore().store(secret, for: reference)
        #expect(try await SecretServiceCredentialStore().secret(for: reference) == secret)
        try await SecretServiceCredentialStore().remove(reference: reference)
        #expect(try await SecretServiceCredentialStore().secret(for: reference) == nil)
        print("Secret Service: store/read across instances/remove verified (secret redacted)")
    }
}
#endif
