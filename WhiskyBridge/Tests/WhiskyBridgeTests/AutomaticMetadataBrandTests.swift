//
//  AutomaticMetadataBrandTests.swift
//  WhiskyBridgeTests
//
//  This file is part of Whisky.
//
//  Whisky is free software: you can redistribute it and/or modify it under the terms
//  of the GNU General Public License as published by the Free Software Foundation,
//  either version 3 of the License, or (at your option) any later version.
//
//  Whisky is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY;
//  without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
//  See the GNU General Public License for more details.
//
//  You should have received a copy of the GNU General Public License along with Whisky.
//  If not, see https://www.gnu.org/licenses/.
//

import Foundation
import Testing
import WhiskyApplication
import WhiskyBridgeProtocol
import WhiskyCore
import WhiskyMetadata
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif

@Test @MainActor func automaticMetadataUsesFolderBrandWhileSearchingCleanTitles() async throws {
    let fixture = try MetadataFixture()
    defer { fixture.remove() }
    BrandSearchProtocol.reset()
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [BrandSearchProtocol.self]
    let session = URLSession(configuration: configuration)
    defer { session.invalidateAndCancel() }

    let folderNames = ["[Key] Matched Story", "[Key] Conflicting Story"]
    for name in folderNames {
        try FileManager.default.createDirectory(
            at: fixture.bottle.appendingPathComponent("Games/\(name)"),
            withIntermediateDirectories: true
        )
    }
    var library = FolderGameLibrary()
    library.automaticMetadata = true
    library.roots = ["Games"]
    library.initializedRoots = ["Games"]
    library.knownFolders = folderNames.map { "Games/\($0)" }
    library.pendingMetadata = Set(library.knownFolders)
    try library.save(bottleURL: fixture.bottle)

    let jobs = BridgeJobStore()
    let service = LibraryMetadataService(jobs: jobs, session: session, networkAccess: {})
    var params = fixture.params
    params["automatic"] = .bool(true)
    params["includeCharacters"] = .bool(false)
    params["sources"] = .array([.string("vndb")])
    let result = try await waitForMetadataJob(
        service.call("metadata.sync", params: params, workspace: fixture.workspace),
        jobs: jobs
    )
    #expect(result.object?["matched"] == .integer(1))
    #expect(result.object?["failed"] == .integer(0))
    #expect(Set(BrandSearchProtocol.terms) == Set(["Matched Story", "Conflicting Story"]))

    let entries = try await fixture.workspace.entries(in: fixture.bottle)
        .filter { $0.source == .folder }
    let matched = try #require(entries.first { $0.name == "[Key] Matched Story" })
    let conflicting = try #require(entries.first { $0.name == "[Key] Conflicting Story" })
    let matchedStore = GameMetadataStore(directory: matched.metadataDirectory, networkAccess: {})
    let conflictingStore = GameMetadataStore(directory: conflicting.metadataDirectory, networkAccess: {})
    #expect(try await matchedStore.load(for: matched.metadataIdentity)?.sourceID == "v1")
    #expect(try await conflictingStore.load(for: conflicting.metadataIdentity) == nil)
}

private final class BrandSearchProtocol: URLProtocol, @unchecked Sendable {
    private static let lock = NSLock()
    private nonisolated(unsafe) static var capturedTerms: [String] = []

    static var terms: [String] {
        lock.withLock { capturedTerms }
    }

    static func reset() {
        lock.withLock { capturedTerms = [] }
    }

    override static func canInit(with request: URLRequest) -> Bool {
        true
    }

    override static func canonicalRequest(for request: URLRequest) -> URLRequest {
        request
    }

    override func stopLoading() {}

    override func startLoading() {
        guard let url = request.url, url.host == "api.vndb.org",
              let data = requestBody(),
              let payload = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let filters = payload["filters"] as? [String], filters.count == 3,
              let response = HTTPURLResponse(url: url, statusCode: 200, httpVersion: nil, headerFields: nil)
        else {
            client?.urlProtocol(self, didFailWithError: URLError(.badServerResponse))
            return
        }
        let term = filters[2]
        let title: String
        let developer: String
        let id: String
        switch (filters[0], term) {
        case ("search", "Matched Story"), ("id", "v1"):
            title = "Matched Story"
            developer = "Key"
            id = "v1"
        case ("search", "Conflicting Story"), ("id", "v2"):
            title = "Conflicting Story"
            developer = "Other Studio"
            id = "v2"
        default:
            client?.urlProtocol(self, didFailWithError: URLError(.badServerResponse))
            return
        }
        if filters[0] == "search" {
            Self.lock.withLock { Self.capturedTerms.append(term) }
        }
        let body = """
        {"results":[{"id":"\(id)","title":"\(title)","developers":[{"name":"\(developer)"}]}]}
        """
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: Data(body.utf8))
        client?.urlProtocolDidFinishLoading(self)
    }

    private func requestBody() -> Data? {
        if let body = request.httpBody {
            return body
        }
        guard let stream = request.httpBodyStream else { return nil }
        stream.open()
        defer { stream.close() }
        var data = Data()
        var buffer = [UInt8](repeating: 0, count: 1_024)
        while stream.hasBytesAvailable {
            let count = stream.read(&buffer, maxLength: buffer.count)
            guard count > 0 else { break }
            data.append(contentsOf: buffer.prefix(count))
        }
        return data
    }
}
