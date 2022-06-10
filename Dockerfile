# Copyright (c) 2019-2020 Wei Tang.
# Copyright (c) 2019 Polkasource.
# SPDX-License-Identifier: Apache-2.0
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#  http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

# ===== START FIRST STAGE ======
#FROM paritytech/ci-linux:974ba3ac-20201006 as builder
#LABEL maintainer "kaisuki@qq.com"
#LABEL description="capsule builder."

# ===== START FIRST STAGE ======
FROM phusion/baseimage:jammy-1.0.0 as builder
LABEL maintainer "team@capsule.ink"
LABEL description="capsule builder."

ARG PROFILE=release
ARG STABLE=nightly
WORKDIR /rustbuilder
COPY . /rustbuilder/capsule

# PREPARE OPERATING SYSTEM & BUILDING ENVIRONMENT
RUN apt-get update && \
	apt-get install -y pkg-config libssl-dev git clang libclang-dev diffutils gcc make m4 build-essential curl file

# UPDATE RUST DEPENDENCIES
ENV RUSTUP_HOME "/rustbuilder/.rustup"
ENV CARGO_HOME "/rustbuilder/.cargo"

RUN curl -sSf https://sh.rustup.rs | sh -s -- --default-toolchain none -y
ENV PATH "$PATH:/rustbuilder/.cargo/bin"
RUN rustup show
RUN rustup update $STABLE

# BUILD RUNTIME AND BINARY
RUN rustup target add wasm32-unknown-unknown --toolchain $STABLE
RUN cd /rustbuilder/capsule && RUSTC_BOOTSTRAP=1 cargo +nightly build --$PROFILE --locked
# ===== END FIRST STAGE ======

# ===== START SECOND STAGE ======
FROM phusion/baseimage:jammy-1.0.0
LABEL maintainer "team@capsule.ink"
LABEL description="capsule binary."
ARG PROFILE=release
COPY --from=builder /rustbuilder/capsule/target/$PROFILE/capsule-node /usr/local/bin

# REMOVE & CLEANUP
RUN mv /usr/share/ca* /tmp && \
	rm -rf /usr/share/*  && \
	mv /tmp/ca-certificates /usr/share/ && \
	rm -rf /usr/lib/python* && \
	mkdir -p /root/.local/share/capsule && \
	ln -s /root/.local/share/capsule /data
RUN	rm -rf /usr/bin /usr/sbin

# FINAL PREPARATIONS
EXPOSE 30333 9933 9944
VOLUME ["/data"]
#CMD ["/usr/local/bin/capsule"]
WORKDIR /usr/local/bin
ENTRYPOINT ["capsule-node"]
#CMD ["--chain=capsule"]
#===== END SECOND STAGE ======