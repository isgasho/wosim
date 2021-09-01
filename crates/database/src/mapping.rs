use crate::{Len, WriteVecGuard};

pub fn add_mapping(
    src: &mut WriteVecGuard<usize>,
    src_index: usize,
    dest: &mut WriteVecGuard<usize>,
) {
    let dest_index = dest.len();
    src[src_index] = dest_index;
    dest.push(src_index);
}

pub fn set_mapping(
    src: &mut WriteVecGuard<usize>,
    src_index: usize,
    dest: &mut WriteVecGuard<usize>,
    dest_index: usize,
) {
    src[src_index] = dest_index;
    dest[dest_index] = src_index;
}

pub fn unset_mapping(
    src: &mut WriteVecGuard<usize>,
    src_index: usize,
    dest: &mut WriteVecGuard<usize>,
) {
    let dest_index = src[src_index];
    src[src_index] = usize::MAX;
    dest[dest_index] = usize::MAX;
}

pub fn remove_mapping(
    src: &mut WriteVecGuard<usize>,
    src_index: usize,
    dest: &mut WriteVecGuard<usize>,
) {
    let dest_index = src[src_index];
    if dest_index + 1 == dest.len() {
        src[dest.pop().unwrap()] = usize::MAX;
    } else {
        src[dest[dest.len() - 1]] = dest_index;
        src[src_index] = usize::MAX;
        dest.copy_within(dest.len() - 1, dest_index);
        dest.pop();
    }
}
