// Copyright 2020 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use encoding::repr::*;
use num_derive::FromPrimitive;

/// ExitCode defines the exit code from the VM execution.
#[repr(u64)]
#[derive(PartialEq, Eq, Debug, Clone, Copy, FromPrimitive, Serialize_repr, Deserialize_repr)]
pub enum ExitCode {
    Ok = 0,
    ErrPlaceholder = 1000,
}

impl ExitCode {
    /// returns true if the exit code was a success
    pub fn is_success(self) -> bool {
        matches!(self, ExitCode::Ok)
    }
}
