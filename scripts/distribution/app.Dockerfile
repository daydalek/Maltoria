# syntax=docker/dockerfile:1
# RHEL 9 family (glibc 2.34) ABI baseline; native amd64 builders are preferred for release CI.
FROM almalinux:9
COPY install-deps.sh /tmp/install-linux-deps.sh
RUN bash /tmp/install-linux-deps.sh && rm /tmp/install-linux-deps.sh
# Official UBI9 Swift toolchain, same version family as the previous noble image.
RUN curl --proto '=https' --tlsv1.2 -sSfL \
      https://download.swift.org/swift-6.4.0-release/ubi9/swift-6.4.0-RELEASE/swift-6.4.0-RELEASE-ubi9.tar.gz \
      -o /tmp/swift.tar.gz \
    && mkdir -p /opt/swift && tar xzf /tmp/swift.tar.gz --strip-components=1 -C /opt/swift \
    && rm /tmp/swift.tar.gz
ENV PATH=/opt/swift/usr/bin:$PATH
ENV RUSTUP_HOME=/opt/rustup CARGO_HOME=/opt/cargo
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o /tmp/rustup.sh \
    && sh /tmp/rustup.sh -y --profile minimal --default-toolchain 1.99.1 \
    && rm /tmp/rustup.sh
ENV PATH=/opt/cargo/bin:$PATH CC=/usr/bin/clang CXX=/usr/bin/clang++
WORKDIR /workspace
