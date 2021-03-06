// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use core::ptr::PtrExt;

// FIXME: #13996: mark the `allocate` and `reallocate` return value as `noalias`

/// Return a pointer to `size` bytes of memory aligned to `align`.
///
/// On failure, return a null pointer.
///
/// Behavior is undefined if the requested size is 0 or the alignment is not a
/// power of 2. The alignment must be no larger than the largest supported page
/// size on the platform.
#[inline]
pub unsafe fn allocate(size: uint, align: uint) -> *mut u8 {
    imp::allocate(size, align)
}

/// Resize the allocation referenced by `ptr` to `size` bytes.
///
/// On failure, return a null pointer and leave the original allocation intact.
///
/// Behavior is undefined if the requested size is 0 or the alignment is not a
/// power of 2. The alignment must be no larger than the largest supported page
/// size on the platform.
///
/// The `old_size` and `align` parameters are the parameters that were used to
/// create the allocation referenced by `ptr`. The `old_size` parameter may be
/// any value in range_inclusive(requested_size, usable_size).
#[inline]
pub unsafe fn reallocate(ptr: *mut u8, old_size: uint, size: uint, align: uint) -> *mut u8 {
    imp::reallocate(ptr, old_size, size, align)
}

/// Resize the allocation referenced by `ptr` to `size` bytes.
///
/// If the operation succeeds, it returns `usable_size(size, align)` and if it
/// fails (or is a no-op) it returns `usable_size(old_size, align)`.
///
/// Behavior is undefined if the requested size is 0 or the alignment is not a
/// power of 2. The alignment must be no larger than the largest supported page
/// size on the platform.
///
/// The `old_size` and `align` parameters are the parameters that were used to
/// create the allocation referenced by `ptr`. The `old_size` parameter may be
/// any value in range_inclusive(requested_size, usable_size).
#[inline]
pub unsafe fn reallocate_inplace(ptr: *mut u8, old_size: uint, size: uint, align: uint) -> uint {
    imp::reallocate_inplace(ptr, old_size, size, align)
}

/// Deallocates the memory referenced by `ptr`.
///
/// The `ptr` parameter must not be null.
///
/// The `old_size` and `align` parameters are the parameters that were used to
/// create the allocation referenced by `ptr`. The `old_size` parameter may be
/// any value in range_inclusive(requested_size, usable_size).
#[inline]
pub unsafe fn deallocate(ptr: *mut u8, old_size: uint, align: uint) {
    imp::deallocate(ptr, old_size, align)
}

/// Returns the usable size of an allocation created with the specified the
/// `size` and `align`.
#[inline]
pub fn usable_size(size: uint, align: uint) -> uint {
    imp::usable_size(size, align)
}

/// Prints implementation-defined allocator statistics.
///
/// These statistics may be inconsistent if other threads use the allocator
/// during the call.
#[unstable]
pub fn stats_print() {
    imp::stats_print();
}

/// An arbitrary non-null address to represent zero-size allocations.
///
/// This preserves the non-null invariant for types like `Box<T>`. The address may overlap with
/// non-zero-size memory allocations.
pub const EMPTY: *mut () = 0x1 as *mut ();

/// The allocator for unique pointers.
#[cfg(not(test))]
#[lang="exchange_malloc"]
#[inline]
unsafe fn exchange_malloc(size: uint, align: uint) -> *mut u8 {
    if size == 0 {
        EMPTY as *mut u8
    } else {
        let ptr = allocate(size, align);
        if ptr.is_null() { ::oom() }
        ptr
    }
}

#[cfg(not(test))]
#[lang="exchange_free"]
#[inline]
unsafe fn exchange_free(ptr: *mut u8, old_size: uint, align: uint) {
    deallocate(ptr, old_size, align);
}

// The minimum alignment guaranteed by the architecture. This value is used to
// add fast paths for low alignment values. In practice, the alignment is a
// constant at the call site and the branch will be optimized out.
#[cfg(any(target_arch = "arm",
          target_arch = "mips",
          target_arch = "mipsel"))]
const MIN_ALIGN: uint = 8;
#[cfg(any(target_arch = "x86",
          target_arch = "x86_64"))]
const MIN_ALIGN: uint = 16;

#[cfg(external_funcs)]
mod imp {
    extern {
        fn rust_allocate(size: uint, align: uint) -> *mut u8;
        fn rust_deallocate(ptr: *mut u8, old_size: uint, align: uint);
        fn rust_reallocate(ptr: *mut u8, old_size: uint, size: uint, align: uint) -> *mut u8;
        fn rust_reallocate_inplace(ptr: *mut u8, old_size: uint, size: uint,
                                   align: uint) -> uint;
        fn rust_usable_size(size: uint, align: uint) -> uint;
        fn rust_stats_print();
    }

    #[inline]
    pub unsafe fn allocate(size: uint, align: uint) -> *mut u8 {
        rust_allocate(size, align)
    }

    #[inline]
    pub unsafe fn reallocate_inplace(ptr: *mut u8, old_size: uint, size: uint,
                                     align: uint) -> uint {
        rust_reallocate_inplace(ptr, old_size, size, align)
    }

    #[inline]
    pub unsafe fn deallocate(ptr: *mut u8, old_size: uint, align: uint) {
        rust_deallocate(ptr, old_size, align)
    }

    #[inline]
    pub unsafe fn reallocate_inplace(ptr: *mut u8, old_size: uint, size: uint,
                                     align: uint) -> uint {
        rust_reallocate_inplace(ptr, old_size, size, align)
    }

    #[inline]
    pub fn usable_size(size: uint, align: uint) -> uint {
        unsafe { rust_usable_size(size, align) }
    }

    #[inline]
    pub fn stats_print() {
        unsafe { rust_stats_print() }
    }
}

#[cfg(external_crate)]
mod imp {
    extern crate external;
    pub use self::external::{allocate, deallocate, reallocate_inplace, reallocate};
    pub use self::external::{usable_size, stats_print};
}

#[cfg(all(not(external_funcs), not(external_crate), jemalloc))]
mod imp {
    use core::option::Option;
    use core::option::Option::None;
    use core::ptr::{null_mut, null};
    use core::num::Int;
    use libc::{c_char, c_int, c_void, size_t};
    use super::MIN_ALIGN;

    #[link(name = "jemalloc", kind = "static")]
    #[cfg(not(test))]
    extern {}

    extern {
        fn je_mallocx(size: size_t, flags: c_int) -> *mut c_void;
        fn je_rallocx(ptr: *mut c_void, size: size_t, flags: c_int) -> *mut c_void;
        fn je_xallocx(ptr: *mut c_void, size: size_t, extra: size_t, flags: c_int) -> size_t;
        fn je_sdallocx(ptr: *mut c_void, size: size_t, flags: c_int);
        fn je_nallocx(size: size_t, flags: c_int) -> size_t;
        fn je_malloc_stats_print(write_cb: Option<extern "C" fn(cbopaque: *mut c_void,
                                                                *const c_char)>,
                                 cbopaque: *mut c_void,
                                 opts: *const c_char);
    }

    // -lpthread needs to occur after -ljemalloc, the earlier argument isn't enough
    #[cfg(all(not(windows), not(target_os = "android")))]
    #[link(name = "pthread")]
    extern {}

    // MALLOCX_ALIGN(a) macro
    #[inline(always)]
    fn mallocx_align(a: uint) -> c_int { a.trailing_zeros() as c_int }

    #[inline(always)]
    fn align_to_flags(align: uint) -> c_int {
        if align <= MIN_ALIGN { 0 } else { mallocx_align(align) }
    }

    #[inline]
    pub unsafe fn allocate(size: uint, align: uint) -> *mut u8 {
        let flags = align_to_flags(align);
        je_mallocx(size as size_t, flags) as *mut u8
    }

    #[inline]
    pub unsafe fn reallocate(ptr: *mut u8, _old_size: uint, size: uint, align: uint) -> *mut u8 {
        let flags = align_to_flags(align);
        je_rallocx(ptr as *mut c_void, size as size_t, flags) as *mut u8
    }

    #[inline]
    pub unsafe fn reallocate_inplace(ptr: *mut u8, _old_size: uint, size: uint,
                                     align: uint) -> uint {
        let flags = align_to_flags(align);
        je_xallocx(ptr as *mut c_void, size as size_t, 0, flags) as uint
    }

    #[inline]
    pub unsafe fn deallocate(ptr: *mut u8, old_size: uint, align: uint) {
        let flags = align_to_flags(align);
        je_sdallocx(ptr as *mut c_void, old_size as size_t, flags)
    }

    #[inline]
    pub fn usable_size(size: uint, align: uint) -> uint {
        let flags = align_to_flags(align);
        unsafe { je_nallocx(size as size_t, flags) as uint }
    }

    pub fn stats_print() {
        unsafe {
            je_malloc_stats_print(None, null_mut(), null())
        }
    }
}

#[cfg(all(not(external_funcs), not(external_crate), not(jemalloc), unix))]
mod imp {
    use core::cmp;
    use core::ptr;
    use libc;
    use super::MIN_ALIGN;

    extern {
        fn posix_memalign(memptr: *mut *mut libc::c_void,
                          align: libc::size_t,
                          size: libc::size_t) -> libc::c_int;
    }

    #[inline]
    pub unsafe fn allocate(size: uint, align: uint) -> *mut u8 {
        if align <= MIN_ALIGN {
            libc::malloc(size as libc::size_t) as *mut u8
        } else {
            let mut out = 0 as *mut libc::c_void;
            let ret = posix_memalign(&mut out,
                                     align as libc::size_t,
                                     size as libc::size_t);
            if ret != 0 {
                ptr::null_mut()
            } else {
                out as *mut u8
            }
        }
    }

    #[inline]
    pub unsafe fn reallocate(ptr: *mut u8, old_size: uint, size: uint, align: uint) -> *mut u8 {
        if align <= MIN_ALIGN {
            libc::realloc(ptr as *mut libc::c_void, size as libc::size_t) as *mut u8
        } else {
            let new_ptr = allocate(size, align);
            ptr::copy_memory(new_ptr, ptr as *const u8, cmp::min(size, old_size));
            deallocate(ptr, old_size, align);
            new_ptr
        }
    }

    #[inline]
    pub unsafe fn reallocate_inplace(_ptr: *mut u8, old_size: uint, _size: uint,
                                     _align: uint) -> uint {
        old_size
    }

    #[inline]
    pub unsafe fn deallocate(ptr: *mut u8, _old_size: uint, _align: uint) {
        libc::free(ptr as *mut libc::c_void)
    }

    #[inline]
    pub fn usable_size(size: uint, _align: uint) -> uint {
        size
    }

    pub fn stats_print() {}
}

#[cfg(all(not(external_funcs), not(external_crate), not(jemalloc), windows))]
mod imp {
    use libc::{c_void, size_t};
    use libc;
    use super::MIN_ALIGN;

    extern {
        fn _aligned_malloc(size: size_t, align: size_t) -> *mut c_void;
        fn _aligned_realloc(block: *mut c_void, size: size_t,
                            align: size_t) -> *mut c_void;
        fn _aligned_free(ptr: *mut c_void);
    }

    #[inline]
    pub unsafe fn allocate(size: uint, align: uint) -> *mut u8 {
        if align <= MIN_ALIGN {
            libc::malloc(size as size_t) as *mut u8
        } else {
            _aligned_malloc(size as size_t, align as size_t) as *mut u8
        }
    }

    #[inline]
    pub unsafe fn reallocate(ptr: *mut u8, _old_size: uint, size: uint, align: uint) -> *mut u8 {
        if align <= MIN_ALIGN {
            libc::realloc(ptr as *mut c_void, size as size_t) as *mut u8
        } else {
            _aligned_realloc(ptr as *mut c_void, size as size_t, align as size_t) as *mut u8
        }
    }

    #[inline]
    pub unsafe fn reallocate_inplace(_ptr: *mut u8, old_size: uint, _size: uint,
                                     _align: uint) -> uint {
        old_size
    }

    #[inline]
    pub unsafe fn deallocate(ptr: *mut u8, _old_size: uint, align: uint) {
        if align <= MIN_ALIGN {
            libc::free(ptr as *mut libc::c_void)
        } else {
            _aligned_free(ptr as *mut c_void)
        }
    }

    #[inline]
    pub fn usable_size(size: uint, _align: uint) -> uint {
        size
    }

    pub fn stats_print() {}
}

#[cfg(test)]
mod test {
    extern crate test;
    use self::test::Bencher;
    use core::ptr::PtrExt;
    use heap;

    #[test]
    fn basic_reallocate_inplace_noop() {
        unsafe {
            let size = 4000;
            let ptr = heap::allocate(size, 8);
            if ptr.is_null() { ::oom() }
            let ret = heap::reallocate_inplace(ptr, size, size, 8);
            heap::deallocate(ptr, size, 8);
            assert_eq!(ret, heap::usable_size(size, 8));
        }
    }

    #[bench]
    fn alloc_owned_small(b: &mut Bencher) {
        b.iter(|| {
            box 10i
        })
    }
}
