import Foundation

#if os(Linux)
struct SecretToolResult: Sendable {
    let status: Int32
    let output: Data
    let diagnostic: Data
}

public actor SecretServiceCredentialStore: CredentialStore {
    typealias Runner = @Sendable ([String], Data?) throws -> SecretToolResult
    private let run: Runner
    private let service: String

    public init(service: String = "com.daydalek.Maltoria.translation") {
        self.service = service
        self.run = Self.execute
    }

    init(service: String = "com.daydalek.Maltoria.translation", runner: @escaping Runner) {
        self.service = service
        self.run = runner
    }

    public func availability() async -> CredentialStoreAvailability { .persistentSecretService }

    public func store(_ secret: String, for reference: String) async throws {
        guard !reference.isEmpty, reference.utf8.count <= 256,
              !secret.isEmpty, secret.utf8.count <= 16_384 else {
            throw TranslationError.credentialUnavailable
        }
        let result = try invoke(["store", "--label=Maltoria translation credential"] + attributes(reference), Data(secret.utf8))
        guard result.status == 0 else { throw TranslationError.credentialOperationFailed(Int(result.status)) }
    }

    public func secret(for reference: String) async throws -> String? {
        let result = try invoke(["lookup"] + attributes(reference))
        if result.status == 1 && result.diagnostic.isEmpty && result.output.isEmpty { return nil }
        guard result.status == 0 else { throw TranslationError.credentialOperationFailed(Int(result.status)) }
        guard var value = String(data: result.output, encoding: .utf8) else {
            throw TranslationError.credentialUnavailable
        }
        if value.hasSuffix("\n") { value.removeLast() }
        return value
    }

    public func remove(reference: String) async throws {
        let result = try invoke(["clear"] + attributes(reference))
        guard result.status == 0 || (result.status == 1 && result.diagnostic.isEmpty) else {
            throw TranslationError.credentialOperationFailed(Int(result.status))
        }
    }

    private func attributes(_ reference: String) -> [String] {
        ["service", service, "account", reference]
    }

    private func invoke(_ arguments: [String], _ input: Data? = nil) throws -> SecretToolResult {
        do { return try run(arguments, input) }
        catch { throw TranslationError.credentialUnavailable }
    }

    private static func execute(_ arguments: [String], _ input: Data?) throws -> SecretToolResult {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/secret-tool")
        process.arguments = arguments
        let stdin = Pipe()
        let stdout = Pipe()
        let stderr = Pipe()
        process.standardInput = stdin
        process.standardOutput = stdout
        process.standardError = stderr
        try process.run()
        if let input { try stdin.fileHandleForWriting.write(contentsOf: input) }
        try stdin.fileHandleForWriting.close()
        let output = stdout.fileHandleForReading.readDataToEndOfFile()
        let diagnostic = stderr.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()
        return SecretToolResult(status: process.terminationStatus, output: output, diagnostic: diagnostic)
    }
}
#endif
