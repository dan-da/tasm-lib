use std::fmt::Display;

use crate::data_type::DataType;
use crate::list::safeimplu32::length::Length as SafeLength;
use crate::list::safeimplu32::new::SafeNew;
use crate::list::safeimplu32::push::SafePush;
use crate::list::safeimplu32::set_length::SafeSetLength;
use crate::list::unsafeimplu32::length::Length as UnsafeLength;
use crate::list::unsafeimplu32::new::UnsafeNew;
use crate::list::unsafeimplu32::push::UnsafePush;
use crate::list::unsafeimplu32::set_length::UnsafeSetLength;
use crate::snippet::BasicSnippet;

pub mod contiguous_list;
pub mod higher_order;
pub mod multiset_equality;
pub mod range;
pub mod safeimplu32;
pub mod unsafeimplu32;

#[derive(Clone, Debug)]
pub enum ListType {
    Safe,
    Unsafe,
}

impl ListType {
    /// the number of words this list type uses for bookkeeping
    pub fn safety_offset(&self) -> usize {
        match self {
            ListType::Safe => 2,
            ListType::Unsafe => 1,
        }
    }

    pub fn new_list(&self, data_type: DataType) -> Box<dyn BasicSnippet> {
        match self {
            ListType::Safe => Box::new(SafeNew { data_type }),
            ListType::Unsafe => Box::new(UnsafeNew { data_type }),
        }
    }

    pub fn push(&self, data_type: DataType) -> Box<dyn BasicSnippet> {
        match self {
            ListType::Safe => Box::new(SafePush { data_type }),
            ListType::Unsafe => Box::new(UnsafePush { data_type }),
        }
    }

    pub fn length(&self, data_type: DataType) -> Box<dyn BasicSnippet> {
        match self {
            ListType::Safe => Box::new(SafeLength { data_type }),
            ListType::Unsafe => Box::new(UnsafeLength { data_type }),
        }
    }

    pub fn set_length(&self, data_type: DataType) -> Box<dyn BasicSnippet> {
        match self {
            ListType::Safe => Box::new(SafeSetLength { data_type }),
            ListType::Unsafe => Box::new(UnsafeSetLength { data_type }),
        }
    }
}

impl Display for ListType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ListType::Safe => write!(f, "safeimplu32"),
            ListType::Unsafe => write!(f, "unsafeimplu32"),
        }
    }
}
