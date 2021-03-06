// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Test that we do not ICE when the self type is `ty::err`, but rather
// just propagate the error.

#![crate_type = "lib"]
#![feature(associated_types, default_type_params, lang_items)]
#![no_std]

#[lang="sized"]
pub trait Sized for Sized? {
    // Empty.
}

#[lang = "add"]
trait Add<RHS=Self> {
    type Output;

    fn add(self, RHS) -> Self::Output;
}

fn ice<A>(a: A) {
    let r = loop {};
    r = r + a; // here the type `r` is not yet inferred, hence `r+a` generates an error.
    //~^ ERROR type of this value must be known
}
