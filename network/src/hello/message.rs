// Copyright 2020 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use anonima_encoding::tuple::*;

/// Hello message https://filecoin-project.github.io/specs/#hello-spec
#[derive(Clone, Debug, PartialEq, Serialize_tuple, Deserialize_tuple)]
pub struct HelloRequest {
    marker: core::marker::PhantomData<()>,
}

/// Response to a Hello message. This just handles latency of the peer.
#[derive(Clone, Debug, PartialEq, Serialize_tuple, Deserialize_tuple)]
pub struct HelloResponse {
    /// Time of arrival to peer in unix nanoseconds.
    pub arrival: u64,
    /// Time sent from peer in unix nanoseconds.
    pub sent: u64,
}
