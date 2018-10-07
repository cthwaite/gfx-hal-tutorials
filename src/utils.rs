use gfx_hal::Backend;
use prelude::*;


pub fn empty_buffer<B: Backend, Item>(device: &B::Device,
                                      memory_types: &[MemoryType],
                                      properties: Properties,
                                      usage: buffer::Usage,
                                      item_count: usize) -> (B::Buffer, B::Memory) {
    let stride = ::std::mem::size_of::<Item>() as u64;
    let buffer_len = item_count as u64 * stride;
    let unbound_buffer = device.create_buffer(buffer_len, usage).unwrap();
    let req = device.get_buffer_requirements(&unbound_buffer);

    let upload_type = memory_types.iter()
                                  .enumerate()
                                  .find(|(id, ty)| {
                                      let type_supported = req.type_mask & (1_u64 << id) != 0;
                                      type_supported && ty.properties.contains(properties)
                                  })
                                  .map(|(id, _ty)| MemoryTypeId(id))
                                  .expect("Could not find appropriate vertex buffer memory type.");
    let buffer_memory = device.allocate_memory(upload_type, req.size).unwrap();
    let buffer = device
        .bind_buffer_memory(&buffer_memory, 0, unbound_buffer)
        .unwrap();

    (buffer, buffer_memory)
}


pub fn fill_buffer<B: Backend, Item: Copy>(device: &B::Device, buffer_memory: &mut B::Memory, items: &[Item]) {
    let stride = ::std::mem::size_of::<Item>() as u64;
    let buffer_len = items.len() as u64 * stride;

    let mut dest = device.acquire_mapping_writer::<Item>(&buffer_memory, 0..buffer_len)
                         .unwrap();
    dest.copy_from_slice(items);
    device.release_mapping_writer(dest);
}


pub fn create_buffer<B: Backend, Item: Copy>(device: &B::Device,
                                      memory_types: &[MemoryType],
                                      properties: Properties,
                                      usage: buffer::Usage,
                                      items: &[Item]) -> (B::Buffer, B::Memory) {
    let (empty_buffer, mut empty_buffer_memory) =
        empty_buffer::<B, Item>(device, memory_types, properties, usage, items.len());

    fill_buffer::<B, Item>(device, &mut empty_buffer_memory, items);

    (empty_buffer, empty_buffer_memory)
}
