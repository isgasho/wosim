use std::{
    ffi::c_void,
    mem::{size_of, swap},
    ops::{Deref, Index},
    ptr::NonNull,
    slice::from_raw_parts,
    sync::Arc,
};

use ash::vk::{self, BufferCreateInfo, BufferUsageFlags, MappedMemoryRange};
use gpu_allocator::{
    vulkan::{Allocation, AllocationCreateDesc},
    MemoryLocation,
};

use crate::{Device, Error};

pub struct Buffer {
    pub(super) handle: vk::Buffer,
    allocation: Option<Allocation>,
    device: Arc<Device>,
}

impl Buffer {
    pub fn new(
        device: Arc<Device>,
        create_info: &BufferCreateInfo,
        location: MemoryLocation,
    ) -> Result<Self, Error> {
        let handle = unsafe { device.inner.create_buffer(create_info, None) }?;
        let requirements = unsafe { device.inner.get_buffer_memory_requirements(handle) };
        let allocation = device
            .allocator
            .lock()
            .unwrap()
            .allocate(&AllocationCreateDesc {
                name: "",
                requirements,
                location,
                linear: true,
            })?;
        unsafe {
            device
                .inner
                .bind_buffer_memory(handle, allocation.memory(), allocation.offset())
        }?;
        Ok(Self {
            handle,
            allocation: Some(allocation),
            device,
        })
    }

    pub fn range(&self) -> MappedMemoryRange {
        self.sub_range(0, self.allocation().size())
    }

    pub fn sub_range(&self, offset: u64, length: u64) -> MappedMemoryRange {
        let allocation = self.allocation();
        let start = self.prev_coherent_multiple(allocation.offset() + offset);
        let end = self.next_coherent_multiple(allocation.offset() + offset + length);
        MappedMemoryRange::builder()
            .memory(unsafe { allocation.memory() })
            .offset(start)
            .size(end - start)
            .build()
    }

    pub fn mapped_ptr(&self) -> Option<NonNull<c_void>> {
        self.allocation().mapped_ptr()
    }

    pub fn next_coherent_multiple(&self, n: u64) -> u64 {
        let m = self.device.non_coherent_atom_size;
        ((n + m - 1) / m) * m
    }

    pub fn prev_coherent_multiple(&self, n: u64) -> u64 {
        let m = self.device.non_coherent_atom_size;
        n - (n % m)
    }

    fn allocation(&self) -> &Allocation {
        self.allocation.as_ref().unwrap()
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        self.device
            .allocator
            .lock()
            .unwrap()
            .free(self.allocation.take().unwrap())
            .unwrap();
        unsafe { self.device.inner.destroy_buffer(self.handle, None) };
    }
}

impl Deref for Buffer {
    type Target = vk::Buffer;

    fn deref(&self) -> &Self::Target {
        &self.handle
    }
}

pub struct GpuVariable<T: Copy> {
    buffer: Buffer,
    ptr: NonNull<T>,
}

impl<T: Copy> GpuVariable<T> {
    pub fn new(
        device: Arc<Device>,
        buffer_usage: BufferUsageFlags,
        location: MemoryLocation,
        value: T,
    ) -> Result<Self, Error> {
        let create_info = BufferCreateInfo::builder()
            .size(size_of::<T>() as u64)
            .usage(buffer_usage);
        let buffer = Buffer::new(device, &create_info, location)?;
        let ptr = buffer.mapped_ptr().unwrap().cast::<T>();
        unsafe { ptr.as_ptr().write(value) };
        Ok(Self { buffer, ptr })
    }

    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub fn range(&self) -> MappedMemoryRange {
        self.buffer.range()
    }

    pub fn value(&self) -> &T {
        unsafe { self.ptr.as_ref() }
    }

    pub fn value_mut(&mut self) -> &mut T {
        unsafe { self.ptr.as_mut() }
    }
}

unsafe impl<T: Copy + Send + Sync> Sync for GpuVariable<T> {}
unsafe impl<T: Copy + Send + Sync> Send for GpuVariable<T> {}

pub struct GpuVec<T: Copy> {
    buffer: Buffer,
    len: usize,
    capacity: usize,
    buffer_usage: BufferUsageFlags,
    location: MemoryLocation,
    ptr: NonNull<T>,
}

impl<T: Copy> GpuVec<T> {
    pub fn new(
        device: Arc<Device>,
        capacity: usize,
        buffer_usage: BufferUsageFlags,
        location: MemoryLocation,
    ) -> Result<Self, Error> {
        assert_ne!(capacity, 0);
        let create_info = BufferCreateInfo::builder()
            .size((size_of::<T>() * capacity) as u64)
            .usage(buffer_usage);
        let buffer = Buffer::new(device, &create_info, location)?;
        let ptr = buffer.mapped_ptr().unwrap().cast::<T>();
        Ok(Self {
            buffer,
            len: 0,
            capacity,
            ptr,
            buffer_usage,
            location,
        })
    }

    pub fn push(&mut self, value: T) {
        assert!(self.len < self.capacity);
        unsafe { self.ptr.as_ptr().add(self.len).write(value) };
        self.len += 1;
    }

    pub fn reserve(&mut self, capacity: usize) -> Result<(), Error> {
        if self.capacity >= capacity {
            return Ok(());
        }
        let create_info = BufferCreateInfo::builder()
            .size((size_of::<T>() * capacity) as u64)
            .usage(self.buffer_usage);
        let mut buffer = Buffer::new(self.buffer.device.clone(), &create_info, self.location)?;
        let mut ptr = buffer.mapped_ptr().unwrap().cast::<T>();
        unsafe {
            ptr.as_ptr()
                .copy_from_nonoverlapping(self.ptr.as_ptr(), self.len)
        };
        swap(&mut self.buffer, &mut buffer);
        swap(&mut self.ptr, &mut ptr);
        self.capacity = capacity;
        Ok(())
    }

    pub fn append(&mut self, values: &[T]) {
        let new_len = self.len + values.len();
        assert!(new_len <= self.capacity);
        unsafe {
            self.ptr
                .as_ptr()
                .add(self.len)
                .copy_from_nonoverlapping(values.as_ptr(), values.len())
        };
        self.len = new_len;
    }

    pub fn as_slice(&self) -> &[T] {
        unsafe { from_raw_parts(self.ptr.as_ptr(), self.len) }
    }

    pub fn range(&self) -> MappedMemoryRange {
        self.buffer.sub_range(0, (self.len * size_of::<T>()) as u64)
    }

    pub fn len(&self) -> usize {
        self.len
    }

    /// # Safety
    pub unsafe fn set_len(&mut self, len: usize) {
        assert!(len <= self.capacity);
        self.len = len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }
}

impl<T: Copy> Index<usize> for GpuVec<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        assert!(index < self.len);
        unsafe { self.ptr.as_ptr().add(index).as_ref().unwrap() }
    }
}

unsafe impl<T: Copy + Send + Sync> Sync for GpuVec<T> {}
unsafe impl<T: Copy + Send + Sync> Send for GpuVec<T> {}
