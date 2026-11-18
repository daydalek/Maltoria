//
//  SourceGatekeeper.swift
//  WhiskyMetadata
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
import WhiskyCore

/// Token-bucket + AIMD pacer shared by `SourceGatekeeper` and `YMGalClient`.
struct AdaptiveTokenBucket: Sendable {
    private static let progressiveCooldowns: [TimeInterval] = [2.0, 5.0, 12.0]
    private static let maxCooldown: TimeInterval = 15.0
    private static let maxAdaptiveInterval: TimeInterval = 4.0

    let minInterval: TimeInterval
    let burstCapacity: Int
    private(set) var adaptiveInterval: TimeInterval
    private(set) var consecutiveRateLimits = 0
    private var tokens: Double
    private var lastRefillAt: Date
    private var lastRequestAt: Date?
    private(set) var cooldownUntil: (date: Date, code: Int)?

    init(minInterval: TimeInterval, burstCapacity: Int = 1) {
        let clampedInterval = max(0, minInterval)
        let clampedBurst = max(1, burstCapacity)
        self.minInterval = clampedInterval
        self.burstCapacity = clampedBurst
        self.adaptiveInterval = clampedInterval
        self.tokens = Double(clampedBurst)
        self.lastRefillAt = Date()
    }

    mutating func clearExpiredCooldown(now: Date = Date()) -> (date: Date, code: Int)? {
        guard let cooldown = cooldownUntil else { return nil }
        if now < cooldown.date {
            return cooldown
        }
        cooldownUntil = nil
        return nil
    }

    mutating func nextWaitDuration(now: Date = Date()) -> TimeInterval {
        guard adaptiveInterval > 0 else { return 0 }
        if burstCapacity <= 1 {
            guard let last = lastRequestAt else { return 0 }
            let elapsed = now.timeIntervalSince(last)
            return elapsed < adaptiveInterval ? (adaptiveInterval - elapsed) : 0
        }
        let elapsed = max(0, now.timeIntervalSince(lastRefillAt))
        tokens = min(Double(burstCapacity), tokens + elapsed / adaptiveInterval)
        lastRefillAt = now
        if tokens >= 1.0 {
            tokens -= 1.0
            return 0
        }
        let wait = (1.0 - tokens) * adaptiveInterval
        tokens = 0
        lastRefillAt = now.addingTimeInterval(wait)
        return wait
    }

    mutating func markRequestFinished() {
        lastRequestAt = Date()
    }

    mutating func recordSuccess() {
        consecutiveRateLimits = 0
        adaptiveInterval = max(minInterval, adaptiveInterval * 0.8)
    }

    mutating func recordRateLimit(_ code: Int, retryAfter: TimeInterval?) {
        if let retryAfter {
            if retryAfter > 0 {
                let until = Date().addingTimeInterval(retryAfter)
                cooldownUntil = (until, code)
                tokens = 0
                lastRefillAt = until
            }
            return
        }
        guard minInterval > 0 else { return }
        consecutiveRateLimits += 1
        let index = min(consecutiveRateLimits - 1, Self.progressiveCooldowns.count - 1)
        let delay = min(Self.maxCooldown, Self.progressiveCooldowns[index])
        let until = Date().addingTimeInterval(delay)
        cooldownUntil = (until, code)
        adaptiveInterval = min(Self.maxAdaptiveInterval, max(adaptiveInterval * 2.0, 0.5))
        tokens = 0
        lastRefillAt = until
    }
}

/// Serializes requests per source, caches recent queries/details, paces burst traffic,
/// and honors HTTP 429/503 Retry-After cooldowns so batch syncs do not hammer sites.
public actor SourceGatekeeper {
    private static let bangumi = SourceGatekeeper(minInterval: 0.10, burstCapacity: 3)
    private static let vndb = SourceGatekeeper(minInterval: 0.12, burstCapacity: 3)
    private static let steam = SourceGatekeeper(minInterval: 0.15, burstCapacity: 2)
    private static let fallback = SourceGatekeeper(minInterval: 0)
    private static let bangumiTransport = SourceGatekeeper(minInterval: 0)
    private static let vndbTransport = SourceGatekeeper(minInterval: 0)
    private static let steamTransport = SourceGatekeeper(minInterval: 0)
    private static let fallbackTransport = SourceGatekeeper(minInterval: 0)

    static func shared(for source: GameMetadataSource) -> SourceGatekeeper {
        switch source {
        case .bangumi: bangumi
        case .vndb: vndb
        case .steam: steam
        case .ymgal, .hikarinagi, .nextmoe: fallback
        }
    }

    static func sharedTransport(for source: GameMetadataSource) -> SourceGatekeeper {
        switch source {
        case .bangumi: bangumiTransport
        case .vndb: vndbTransport
        case .steam: steamTransport
        case .ymgal, .hikarinagi, .nextmoe: fallbackTransport
        }
    }

    private var pacer: AdaptiveTokenBucket
    private var busy = false
    private var waiters: [CheckedContinuation<Void, Never>] = []
    private var searches: [String: (Date, [GameMetadata])] = [:]
    private var gameDetails: [String: (Date, GameMetadata)] = [:]
    private var characterResults: [String: (Date, [GameCharacter])] = [:]
    private var characterFailures: [String: (Date, Error)] = [:]

    var adaptiveInterval: TimeInterval {
        pacer.adaptiveInterval
    }

    var consecutiveRateLimits: Int {
        pacer.consecutiveRateLimits
    }

    var cooldownDate: Date? {
        pacer.cooldownUntil?.date
    }

    public init(minInterval: TimeInterval, burstCapacity: Int = 1) {
        self.pacer = AdaptiveTokenBucket(minInterval: minInterval, burstCapacity: burstCapacity)
    }

    private func acquire() async throws {
        if busy {
            await withCheckedContinuation { waiters.append($0) }
        } else {
            busy = true
        }
        do { try Task.checkCancellation() } catch { release()
            throw error
        }
    }

    private func release() {
        if waiters.isEmpty {
            busy = false
        } else {
            waiters.removeFirst().resume()
        }
    }

    private func paceAndCheckCooldown() async throws {
        if let cooldown = pacer.clearExpiredCooldown() {
            throw ProviderError.http(cooldown.code)
        }
        let delay = pacer.nextWaitDuration()
        if delay > 0 {
            try await Task.sleep(for: .milliseconds(Int((delay * 1_000).rounded(.up))))
        }
    }

    public func search(
        _ key: String,
        operation: @Sendable () async throws -> [GameMetadata]
    ) async throws -> [GameMetadata] {
        try await acquire()
        defer { release() }
        let now = Date()
        if let cached = searches[key], now.timeIntervalSince(cached.0) < 300 {
            return cached.1
        }
        try await paceAndCheckCooldown()
        defer { pacer.markRequestFinished() }
        do {
            let results = try await operation()
            pacer.recordSuccess()
            Self.insertCache(results, forKey: key, into: &searches, ttl: 300, limit: 64)
            return results
        } catch let ProviderError.rateLimited(code, retryAfter) {
            pacer.recordRateLimit(code, retryAfter: retryAfter)
            throw ProviderError.http(code)
        }
    }

    public func details(
        _ id: String,
        operation: @Sendable () async throws -> GameMetadata
    ) async throws -> GameMetadata {
        try await acquire()
        defer { release() }
        let now = Date()
        if let cached = gameDetails[id], now.timeIntervalSince(cached.0) < 600 {
            return cached.1
        }
        try await paceAndCheckCooldown()
        defer { pacer.markRequestFinished() }
        do {
            let result = try await operation()
            pacer.recordSuccess()
            Self.insertCache(result, forKey: id, into: &gameDetails, ttl: 600, limit: 100)
            return result
        } catch let ProviderError.rateLimited(code, retryAfter) {
            pacer.recordRateLimit(code, retryAfter: retryAfter)
            throw ProviderError.http(code)
        }
    }

    /// A serialized, paced operation whose result belongs to the caller's local store.
    /// Rethrows `ProviderError.rateLimited` after recording local cooldown so outer
    /// gatekeepers can apply AIMD backoff as well.
    public func paced<T: Sendable>(operation: @Sendable () async throws -> T) async throws -> T {
        try await acquire()
        defer { release() }
        try await paceAndCheckCooldown()
        defer { pacer.markRequestFinished() }
        do {
            let result = try await operation()
            pacer.recordSuccess()
            return result
        } catch let ProviderError.rateLimited(code, retryAfter) {
            pacer.recordRateLimit(code, retryAfter: retryAfter)
            throw ProviderError.rateLimited(code, retryAfter: retryAfter)
        }
    }

    public func characters(
        _ id: String,
        operation: @Sendable () async throws -> [GameCharacter]
    ) async throws -> [GameCharacter] {
        try await acquire()
        defer { release() }
        if let cached = characterResults[id], Date().timeIntervalSince(cached.0) < 600 {
            return cached.1
        }
        if let failed = characterFailures[id], Date().timeIntervalSince(failed.0) < 30 {
            throw failed.1
        }
        try await paceAndCheckCooldown()
        defer { pacer.markRequestFinished() }
        do {
            let result = try await operation()
            pacer.recordSuccess()
            Self.insertCache(result, forKey: id, into: &characterResults, ttl: 600, limit: 100)
            characterFailures[id] = nil
            return result
        } catch let ProviderError.rateLimited(code, retryAfter) {
            pacer.recordRateLimit(code, retryAfter: retryAfter)
            throw ProviderError.http(code)
        } catch {
            if !Task.isCancelled {
                Self.insertCache(error, forKey: id, into: &characterFailures, ttl: 30, limit: 100)
            }
            throw error
        }
    }

    static func insertCache<Value>(
        _ value: Value, forKey key: String, into cache: inout [String: (Date, Value)],
        ttl: TimeInterval, limit: Int
    ) {
        let now = Date()
        if cache.count >= limit {
            cache = cache.filter { now.timeIntervalSince($0.value.0) < ttl }
            if cache.count >= limit, let oldestKey = cache.min(by: { $0.value.0 < $1.value.0 })?.key {
                cache.removeValue(forKey: oldestKey)
            }
        }
        cache[key] = (now, value)
    }
}
