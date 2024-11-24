// Copyright (C) 2024 Tristan Gerritsen <tristan@thewoosh.org>
// All Rights Reserved.

use std::collections::HashMap;

use babbelaar::BabString;
use object::{RelocationEncoding, RelocationFlags, RelocationKind};

use super::aarch64::ArmInstruction;

#[derive(Debug, Clone)]
pub struct FunctionLink {
    pub(super) name: BabString,
    pub(super) offset: usize,
    pub(super) method: FunctionLinkMethod,
}

impl FunctionLink {
    #[must_use]
    pub fn name(&self) -> &BabString {
        &self.name
    }

    #[must_use]
    pub fn offset(&self) -> usize {
        self.offset
    }

    #[must_use]
    pub(crate) fn flags(&self) -> RelocationFlags {
        RelocationFlags::Generic {
            kind: RelocationKind::PltRelative,
            encoding: self.method.encoding(),
            size: self.method.size(),
        }
    }

    pub(crate) fn addend(&self) -> i64 {
        self.method.addend()
    }

    pub fn write(&self, code: &mut [u8], offset: isize) {
        let code = &mut code[self.offset..];

        match self.method {
            FunctionLinkMethod::AArch64BranchLink => {
                let offset = (offset / 4) - 1;

                let instruction = ArmInstruction::Bl { offset: offset as i32, symbol_name: BabString::empty() }
                    .encode(0, &HashMap::new());
                code[0..4].copy_from_slice(&instruction.to_ne_bytes());
            }

            FunctionLinkMethod::Amd64CallNearRelative => {
                let offset = (offset - self.offset as isize - 5) as u32;
                code[1..5].copy_from_slice(&offset.to_le_bytes());
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum FunctionLinkMethod {
    AArch64BranchLink,
    Amd64CallNearRelative,
}

impl FunctionLinkMethod {
    #[must_use]
    pub fn encoding(&self) -> RelocationEncoding {
        match self {
            Self::AArch64BranchLink => RelocationEncoding::AArch64Call,
            Self::Amd64CallNearRelative => RelocationEncoding::X86RipRelative,
        }
    }

    #[must_use]
    pub fn size(&self) -> u8 {
        match self {
            Self::AArch64BranchLink => 32,
            Self::Amd64CallNearRelative => 40,
        }
    }

    #[must_use]
    pub fn addend(&self) -> i64 {
        match self {
            Self::AArch64BranchLink => 0,
            Self::Amd64CallNearRelative => 0,
        }
    }
}
