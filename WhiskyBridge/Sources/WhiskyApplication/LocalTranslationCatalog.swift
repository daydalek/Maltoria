//
//  LocalTranslationCatalog.swift
//  WhiskyApplication
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
import WhiskyTranslation

public struct LocalRuntimeAsset: Codable, Equatable, Sendable {
    public let version: String
    public let platform: String
    public let filename: String
    public let downloadURL: String
    public let sizeBytes: Int64
    public let sha256: String
    public let gpuAcceleration: String

    public init(
        version: String,
        platform: String,
        filename: String,
        downloadURL: String,
        sizeBytes: Int64,
        sha256: String,
        gpuAcceleration: String
    ) {
        self.version = version
        self.platform = platform
        self.filename = filename
        self.downloadURL = downloadURL
        self.sizeBytes = sizeBytes
        self.sha256 = sha256
        self.gpuAcceleration = gpuAcceleration
    }
}

public struct LocalModelCatalogEntry: Codable, Equatable, Identifiable, Sendable {
    public let id: String
    public let displayName: String
    public let subtitle: String
    public let recommended: Bool
    public let repoID: String
    public let filename: String
    public let sizeBytes: Int64
    public let sha256: String
    public let contextWindow: Int
    public let recommendedRamGB: Int
    public let licenseName: String
    public let licenseURL: String
    /// llama-server `-c` override; nil launches with `contextWindow`. Keeps the
    /// runtime's KV cache allocation within `recommendedRamGB` for models that
    /// advertise a large window.
    public let serverContextWindow: Int?

    public init(
        id: String,
        displayName: String,
        subtitle: String,
        recommended: Bool,
        repoID: String,
        filename: String,
        sizeBytes: Int64,
        sha256: String,
        contextWindow: Int,
        recommendedRamGB: Int,
        licenseName: String,
        licenseURL: String,
        serverContextWindow: Int? = nil
    ) {
        self.id = id
        self.displayName = displayName
        self.subtitle = subtitle
        self.recommended = recommended
        self.repoID = repoID
        self.filename = filename
        self.sizeBytes = sizeBytes
        self.sha256 = sha256
        self.contextWindow = contextWindow
        self.recommendedRamGB = recommendedRamGB
        self.licenseName = licenseName
        self.licenseURL = licenseURL
        self.serverContextWindow = serverContextWindow
    }

    public var modelDescriptor: ModelDescriptor {
        ModelDescriptor(
            id: id,
            displayName: displayName,
            contextWindow: contextWindow,
            contextWindowProvenance: .detected,
            capabilities: [.textCompletion, .streaming]
        )
    }

    public func downloadURL(
        useChinaMirror: Bool,
        environment: [String: String] = ProcessInfo.processInfo.environment
    ) -> URL? {
        let base = LocalTranslationCatalog.resolvedHuggingFaceBase(
            useChinaMirror: useChinaMirror,
            environment: environment
        )
        let trimmedBase = base.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        return URL(string: "\(trimmedBase)/\(repoID)/resolve/main/\(filename)?download=true")
    }
}

public struct LocalTranslationCatalog: Sendable {
    public static let defaultHuggingFaceBase = "https://huggingface.co"
    public static let chinaHuggingFaceMirror = "https://hf-mirror.com"
    public static let pinnedRuntimeVersion = "b12540"

    public let runtimeAsset: LocalRuntimeAsset
    public let models: [LocalModelCatalogEntry]

    public init(runtimeAsset: LocalRuntimeAsset, models: [LocalModelCatalogEntry]) {
        self.runtimeAsset = runtimeAsset
        self.models = models
    }

    public static func `default`(
        environment: [String: String] = ProcessInfo.processInfo.environment
    ) -> LocalTranslationCatalog {
        LocalTranslationCatalog(
            runtimeAsset: defaultRuntimeAsset(environment: environment),
            models: defaultModels
        )
    }

    public static let defaultModels: [LocalModelCatalogEntry] = [
        LocalModelCatalogEntry(
            id: "plamo-3-translate-2b-q4_k_m",
            displayName: "PLaMo 3 Translate 2B (Q4_K_M)",
            subtitle: "推荐主力 · 日中准确度、人称指代与语境理解更强（运行约占 2.2~2.7 GB 内存）",
            recommended: true,
            repoID: "pfnet/plamo-3-translate-2b-GGUF",
            filename: "plamo-3-translate-2b-Q4_K_M.gguf",
            sizeBytes: 1_807_203_040,
            sha256: "1ffd9073afcc70e35ccc942428aa2b73d7aa2b9ea779bf504b775369e0276c13",
            contextWindow: 32_768,
            recommendedRamGB: 8,
            serverContextWindow: 4_096,
            licenseName: "PLaMo Community License",
            licenseURL: "https://plamo.preferredai.jp/info/plamo-community-license-en"
        ),
        LocalModelCatalogEntry(
            id: "plamo-3-translate-0.8b-q4_k_m",
            displayName: "PLaMo 3 Translate 0.8B (Q4_K_M)",
            subtitle: "轻量极速 · 下载更快、延迟更低，适合短句或低内存设备（运行约占 1.0 GB 内存）",
            recommended: false,
            repoID: "pfnet/plamo-3-translate-0.8b-GGUF",
            filename: "plamo-3-translate-0.8b-Q4_K_M.gguf",
            sizeBytes: 573_126_848,
            sha256: "70891ec70dc7ab4205745a38307867c2881b99239f20327927633abfe4bc9ec6",
            contextWindow: 32_768,
            recommendedRamGB: 4,
            serverContextWindow: 4_096,
            licenseName: "PLaMo Community License",
            licenseURL: "https://plamo.preferredai.jp/info/plamo-community-license-en"
        ),
        LocalModelCatalogEntry(
            id: "hy-mt3-1.8b-q4_k_m",
            displayName: "Hy-MT3 1.8B (Q4_K_M)",
            subtitle: "腾讯混元 MT3 · 覆盖 102 种语言，指令遵循与上下文理解更强（运行约占 1.8~2.3 GB 内存）",
            recommended: false,
            repoID: "tencent/Hy-MT3-1.8B-GGUF",
            filename: "Hy-MT3-1.8B-Q4_K_M.gguf",
            sizeBytes: 1_348_432_160,
            sha256: "bae6e591103654ff43ef0de7d1492f6e6df1f955505f6bb2cc52e86fc399a942",
            contextWindow: 8_192,
            recommendedRamGB: 8,
            licenseName: "Apache-2.0",
            licenseURL: "https://huggingface.co/tencent/Hy-MT3-1.8B-GGUF/blob/main/LICENSE.txt"
        )
    ]

    public static func defaultRuntimeAsset(
        environment: [String: String] = ProcessInfo.processInfo.environment
    ) -> LocalRuntimeAsset {
        let basePrefix: String = if let raw = environment["MALTORIA_LLAMA_DOWNLOAD_BASE"]?
            .trimmingCharacters(in: .whitespacesAndNewlines),
            !raw.isEmpty {
            raw.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        } else {
            "https://github.com/ggml-org/llama.cpp/releases/download/\(pinnedRuntimeVersion)"
        }
        #if os(macOS)
        let filename = "llama-\(pinnedRuntimeVersion)-bin-macos-arm64.tar.gz"
        return LocalRuntimeAsset(
            version: pinnedRuntimeVersion,
            platform: "macos-arm64",
            filename: filename,
            downloadURL: "\(basePrefix)/\(filename)",
            sizeBytes: 16_443_903,
            sha256: "3cd5875903cf7f87f85f0717177e1fdcdbfb3f54163e2c2b67009d4709e7b79e",
            gpuAcceleration: "metal"
        )
        #else
        let useVulkan = FileManager.default.fileExists(atPath: "/usr/lib/x86_64-linux-gnu/libvulkan.so.1")
            || FileManager.default.fileExists(atPath: "/usr/lib64/libvulkan.so.1")
            || FileManager.default.fileExists(atPath: "/usr/lib/libvulkan.so.1")
        if useVulkan {
            let filename = "llama-\(pinnedRuntimeVersion)-bin-ubuntu-vulkan-x64.tar.gz"
            return LocalRuntimeAsset(
                version: pinnedRuntimeVersion,
                platform: "linux-x64-vulkan",
                filename: filename,
                downloadURL: "\(basePrefix)/\(filename)",
                sizeBytes: 36_669_205,
                sha256: "ec85327bf9ded0e65b66138c59db8eb0f9869b4c77057902e8059d3afeb0bfad",
                gpuAcceleration: "vulkan"
            )
        } else {
            let filename = "llama-\(pinnedRuntimeVersion)-bin-ubuntu-x64.tar.gz"
            return LocalRuntimeAsset(
                version: pinnedRuntimeVersion,
                platform: "linux-x64-cpu",
                filename: filename,
                downloadURL: "\(basePrefix)/\(filename)",
                sizeBytes: 23_010_544,
                sha256: "804bed1f58978c3577751bd9cbdfb8c17ee23c934e5afa82be561dfed0ac18fa",
                gpuAcceleration: "cpu"
            )
        }
        #endif
    }

    public static func resolvedHuggingFaceBase(
        useChinaMirror: Bool,
        environment: [String: String] = ProcessInfo.processInfo.environment
    ) -> String {
        if let override = environmentOverrideHFEndpoint(environment: environment) {
            return override
        }
        return useChinaMirror ? chinaHuggingFaceMirror : defaultHuggingFaceBase
    }

    public static func environmentOverrideHFEndpoint(
        environment: [String: String] = ProcessInfo.processInfo.environment
    ) -> String? {
        for key in ["MALTORIA_HF_ENDPOINT", "HF_ENDPOINT"] {
            if let raw = environment[key]?.trimmingCharacters(in: .whitespacesAndNewlines),
               !raw.isEmpty,
               let url = URL(string: raw),
               let scheme = url.scheme?.lowercased(),
               scheme == "https" || scheme == "http" {
                return raw.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
            }
        }
        return nil
    }

    public func model(id: String) -> LocalModelCatalogEntry? {
        models.first { $0.id == id }
    }
}
