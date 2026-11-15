// swift-tools-version: 6.0
//
//  WhiskyTranslation
//
//  This file is part of Maltoria.
//
//  Maltoria is free software: you can redistribute it and/or modify it under the terms
//  of the GNU General Public License as published by the Free Software Foundation,
//  either version 3 of the License, or (at your option) any later version.
//
//  Maltoria is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY;
//  without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
//  See the GNU General Public License for more details.
//
//  You should have received a copy of the GNU General Public License along with Maltoria.
//  If not, see https://www.gnu.org/licenses/.
//

import PackageDescription

let package = Package(
    name: "WhiskyTranslation",
    platforms: [.macOS(.v15)],
    products: [.library(name: "WhiskyTranslation", targets: ["WhiskyTranslation"])],
    targets: [
        .target(
            name: "WhiskyTranslation",
            linkerSettings: [.linkedFramework("Security", .when(platforms: [.macOS]))]
        ),
        .testTarget(name: "WhiskyTranslationTests", dependencies: ["WhiskyTranslation"])
    ],
    swiftLanguageModes: [.v6]
)
