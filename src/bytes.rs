pub fn uninitialized(len: usize, align: usize) -> Box<[u8]> {
    let layout = std::alloc::Layout::from_size_align(len, align).expect("cannot build Layout");
    unsafe {
        Box::from_raw(std::ptr::slice_from_raw_parts_mut(
            std::alloc::alloc(layout),
            len,
        ))
    }
}
