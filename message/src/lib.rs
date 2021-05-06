// Copyright 2020 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

// workaround for a compiler bug, see https://github.com/rust-lang/rust/issues/55779
extern crate serde;

pub mod exit_code;
pub mod message_receipt;
pub mod method;
pub mod signed_message;
pub mod unsigned_message;

pub use exit_code::*;
pub use message_receipt::MessageReceipt;
pub use method::*;
pub use signed_message::SignedMessage;
pub use unsigned_message::UnsignedMessage;

use address::Address;

/// Message interface to interact with Signed and unsigned messages in a generic
/// context.
pub trait Message {
    /// Returns the from address of the message.
    fn from(&self) -> &Address;
    /// Returns the destination address of the message.
    fn to(&self) -> &Address;
    /// Returns the method number to be called.
    fn method_num(&self) -> MethodNum;
    /// Returns the encoded parameters for the method call.
    fn params(&self) -> &Serialized;
}
