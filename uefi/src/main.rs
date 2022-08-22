#![feature(abi_efiapi)]
#![no_std]
#![no_main]

extern crate alloc;

use core::alloc::Layout;

use alloc::{alloc::alloc_zeroed, vec, vec::Vec};
use uefi::{
    proto::media::file::{File, FileAttribute, FileMode},
    CStr16,
};
use xmas_elf::{
    header::Class,
    program::{ProgramHeader, Type},
};
use zero::read;

cfg_if::cfg_if! {
    if #[cfg(target_pointer_width = "32")] {
        const FILE_HEADER_SIZE: usize = 0x32;
        const PROGRAM_HEADER_SIZE: usize = 0x20;
    } else if #[cfg(target_pointer_width = "64")] {
        const FILE_HEADER_SIZE: usize = 0x40;
        const PROGRAM_HEADER_SIZE: usize = 0x38;
    } else {
        compile_error!("unsupported pointer width");
    }
}

// Separating main into multiple smaller functions isn't trivial because
// xmas_elf uses backing buffers extensively. This could be worked around using
// Pin, but kinda not worth it.

#[uefi::prelude::entry]
fn main(
    handle: uefi::Handle,
    mut system_table: uefi::table::SystemTable<uefi::table::Boot>,
) -> uefi::Status {
    macro_rules! u {
        ($($tt:tt)*) => {
            usize::try_from($($tt)*).unwrap()
        };
    }

    uefi_services::init(&mut system_table).unwrap();
    let proto_ptr = system_table
        .boot_services()
        .get_image_file_system(handle)
        .unwrap()
        .interface
        .get();
    let proto = unsafe { &mut *proto_ptr };
    let mut root = proto.open_volume().unwrap();

    const KERNEL_ELF: &str = "kernel.elf";

    // Extra null byte.
    let mut bytes = [0; KERNEL_ELF.len() + 1];
    let cstr = CStr16::from_str_with_buf(KERNEL_ELF, &mut bytes).unwrap();

    log::info!("opening kernel elf");

    let mut file = root
        .open(cstr, FileMode::Read, FileAttribute::READ_ONLY)
        .unwrap()
        .into_regular_file()
        .unwrap();

    log::info!("successfuly opened kernel elf");

    let mut bytes = [0; FILE_HEADER_SIZE];

    let bytes_read = file.read(&mut bytes).unwrap();
    assert_eq!(bytes_read, bytes.len());

    let file_header = xmas_elf::header::parse_header(&bytes).unwrap();

    assert_eq!(file_header.pt2.ph_entry_size(), PROGRAM_HEADER_SIZE as u16);

    let ph_entry_size = file_header.pt2.ph_entry_size();
    let ph_count = file_header.pt2.ph_count();
    log::info!("kernel elf contains {ph_count} program headers");

    file.set_position(file_header.pt2.ph_offset()).unwrap();
    let mut bytes = vec![0; usize::from(ph_count) * usize::from(ph_entry_size)];
    let bytes_read = file.read(&mut bytes).unwrap();
    assert_eq!(bytes_read, bytes.len());

    let mut program_headers = Vec::with_capacity(ph_count.into());
    for i in 0..ph_count {
        let start = usize::from(i) * usize::from(ph_entry_size);
        let end = (usize::from(i) + 1) * usize::from(ph_entry_size);
        let program_header = match file_header.pt1.class() {
            Class::ThirtyTwo => ProgramHeader::Ph32(read(&bytes[start..end])),
            Class::SixtyFour => ProgramHeader::Ph64(read(&bytes[start..end])),
            _ => panic!("unknown elf class"),
        };
        program_headers.push(program_header);
    }

    // TODO: Do we have to use the same page size as the kernel?
    let page_size: usize = 1 << 12;
    // Needed for our fancy alignment algorithm.
    assert!(page_size.is_power_of_two());

    let mut alignment = page_size;
    let mut image_start = usize::MAX;
    let mut image_end = 0;

    for header in program_headers.iter() {
        if header.get_type().unwrap() != Type::Load {
            continue;
        }

        let header_align = u!(header.align());
        if header_align > alignment {
            // FIXME: would we need to realign previous programs?
            alignment = header_align;
        }

        let header_start = {
            let start = u!(header.virtual_addr());
            // Round down.
            start & !(alignment - 1)
        };

        if image_start > header_start {
            image_start = header_start;
        }

        let header_end = {
            let start = u!(header.virtual_addr());
            let mem_size = u!(header.mem_size());
            let end = start + mem_size;
            // Round up.
            (end + alignment - 1) & !(alignment - 1)
        };

        if image_end < header_end {
            image_end = header_end;
        }
    }

    log::info!("start: {image_start:#0x}");
    log::info!("end: {image_end:#0x}");
    log::info!("alignment: {alignment:#0x}");

    let size = image_end - image_start;

    let layout = Layout::from_size_align(size, alignment).unwrap();
    let image_address = unsafe { alloc_zeroed(layout) };

    let image_entry = image_address as usize + u!(file_header.pt2.entry_point()) - image_start;

    log::info!("loading programs");

    for header in program_headers {
        if header.get_type().unwrap() != Type::Load {
            continue;
        }

        let header_address =
            (image_address as usize + u!(header.virtual_addr()) - image_start) as *mut u8;
        let header_mem =
            unsafe { core::slice::from_raw_parts_mut(header_address, u!(header.file_size())) };

        file.set_position(header.offset()).unwrap();
        let bytes_read = file.read(header_mem).unwrap();
        assert_eq!(bytes_read, header_mem.len());
    }

    let mmap_size = system_table.boot_services().memory_map_size().map_size;
    let mut mmap = vec![0; mmap_size];

    log::info!("exiting UEFI boot services");
    system_table.exit_boot_services(handle, &mut mmap).unwrap();

    let _start: extern "C" fn() -> ! = unsafe { core::mem::transmute(image_entry) };
    _start();
}
