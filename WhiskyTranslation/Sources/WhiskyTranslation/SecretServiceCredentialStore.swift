import Foundation

#if os(Linux)
import Synchronization

struct SecretToolResult: Sendable {
    let status: Int32
    let output: Data
    let diagnostic: Data
}

public actor SecretServiceCredentialStore: CredentialStore {
    typealias Runner = @Sendable ([String], Data?) throws -> SecretToolResult
    private let run: Runner
    private let service: String
    private let executableAvailable: @Sendable () -> Bool

    public init(service: String = "com.daydalek.Maltoria.translation", executableURL: URL = URL(fileURLWithPath: "/usr/bin/secret-tool")) {
        self.service = service
        self.run = { try Self.execute($0, $1, executableURL: executableURL) }
        self.executableAvailable = { FileManager.default.isExecutableFile(atPath: executableURL.path) }
    }

    init(service: String = "com.daydalek.Maltoria.translation", runner: @escaping Runner) {
        self.service = service
        self.run = runner
        self.executableAvailable = { true }
    }

    public func availability() async -> CredentialStoreAvailability {
        executableAvailable() ? .persistentSecretService : .unavailable
    }

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

    static func execute(_ arguments: [String], _ input: Data?, executableURL: URL = URL(fileURLWithPath: "/usr/bin/secret-tool")) throws -> SecretToolResult {
        let process = Process()
        process.executableURL = executableURL
        process.arguments = arguments
        let stdin = Pipe()
        let stdout = Pipe()
        let stderr = Pipe()
        process.standardInput = stdin
        process.standardOutput = stdout
        process.standardError = stderr
        try process.run()
        let readers = DispatchGroup()
        let output = Mutex(Data())
        let diagnostic = Mutex(Data())
        DispatchQueue.global().async(group: readers) {
            output.withLock { $0 = stdout.fileHandleForReading.readDataToEndOfFile() }
        }
        DispatchQueue.global().async(group: readers) {
            diagnostic.withLock { $0 = stderr.fileHandleForReading.readDataToEndOfFile() }
        }
        if let input { try stdin.fileHandleForWriting.write(contentsOf: input) }
        try stdin.fileHandleForWriting.close()
        process.waitUntilExit()
        readers.wait()
        return SecretToolResult(status: process.terminationStatus, output: output.withLock { $0 }, diagnostic: diagnostic.withLock { $0 })
    }
}
#endif
