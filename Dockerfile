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
FROM phusion/baseimage:0.11 as builder
LABEL maintainer "kaisuki@qq.com"
LABEL description="capsule builder."

ARG PROFILE=release
ARG STABLE=nightly-2021-09-12
WORKDIR /rustbuilder
COPY . /rustbuilder/capsule

# PREPARE OPERATING SYSTEM & BUILDING ENVIRONMENT
RUN apt-get update
RUN apt install curl build-essential gcc make libcurl4 libssl1.1 -y
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
