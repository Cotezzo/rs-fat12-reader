use std::{fs::File, io::{self, Seek, SeekFrom}, io::Read, mem};

/* ==== STRUCTS ============================================================= */
/** Define FAT12 headers and bootloader sector.
 *  All the header values are mapped, but the bootloader code is ignored. */
 // repr(C): ensures that the data layout is laid in "the C way" for FFI (Foreign Function Interface)
 // repr(packed): ensures that no padding data is added between struct fields
 #[repr(C, packed)]
 #[derive(Debug)]
pub struct BootSector {
    // BIOS Parameter Block
    pub jump_instruction: [u8; 3],
    pub oem_id: [u8; 8],
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub reserved_sectors: u16,
    pub fat_count: u8,
    pub root_entries : u16,
    pub sector_count: u16,
    pub media_descriptor: u8,
    pub sectors_per_fat: u16,
    pub sectors_per_cylinder: u16,
    pub heads_count: u16,
    pub hidden_sectors_count: u32,
    pub large_sector_count: u32,

    // Extended Boot Record
    pub drive_number: u8,
    pub reserved: u8,
    pub volume_id : u32,
    pub volume_label : [u8; 11],
    pub system_id: [u8; 8]

    // BootLoader code (ignored)
}

impl BootSector {
    pub fn get_fat_start(&self) -> u16 {
        self.reserved_sectors * self.bytes_per_sector
    }

    pub fn get_fat_size(&self) -> u16 {
        self.sectors_per_fat * self.bytes_per_sector
    }

    pub fn get_root_dir_start(&self) -> u16 {
        self.get_fat_start() + (self.get_fat_size() * self.fat_count as u16)
    }

    pub fn get_root_dir_size(&self) -> usize {
        self.root_entries as usize * std::mem::size_of::<DirectoryEntry>()
    }

    pub fn get_cluster_region_start(&self) -> usize {
        self.get_root_dir_start() as usize + self.get_root_dir_size()
    }

    pub fn get_cluster_start(&self, cluster: u16) -> usize {
        self.get_cluster_region_start() + (self.get_cluster_size() * (cluster - 2) as usize)
    }

    pub fn get_cluster_size(&self) -> usize {
        self.sectors_per_cluster as usize * self.bytes_per_sector as usize
    }
}

pub struct Fat {
    entries: Vec<u8>

    // ! Readonly (immutable reference)
    // entries: &'static[u8]

    // ! Unsafe if memory allocation is not handled, memory can be overwritten
    //entries: *const u8
}

impl Fat{
    pub fn get_entry(&self, cluster: usize) -> u16 {
        //! Unsafe: we're not checking FAT size against input cluster

        // Get single byte position and find index array (element = 2B)
        let i: usize = cluster * 3 / 2;

        // Get 4 if the reminder is 1 (odd number), 0 otherwise (even number)
        // This number is used for bitshifting by half byte
        let c: usize = ((cluster * 3) % 2) * 4;
        
        // First element contains the least significant byte
        // If the reminder is odd, we only need the upper 4 bits
        let lsb: u8 = unsafe { self.entries.get(i).unwrap_unchecked() } & (0xFF << c);

        // Second element contains the most significant byte
        // If the reminder is even, we only need the lower 4 bits
        let msb: u8 = unsafe { self.entries.get(i+1).unwrap_unchecked() } & (0xFF >> (4-c));

        // "Concat" the two bytes in a word
        let word: u16 = ((msb as u16) * 256) + lsb as u16;

        // If the reminder is odd, the entry is in the upper 12bits, right shift
        // If the reminder is even, we need to remove the upper 4bits
        (word >> c) & 0x0FFF
    }
}

#[repr(C, packed)]
#[derive(Debug)]
pub struct DirectoryEntry {
    pub name: [u8; 11],
    pub attributes: u8,             // READ_ONLY=0x01 HIDDEN=0x02 SYSTEM=0x04 VOLUME_ID=0x08 DIRECTORY=0x10 ARCHIVE=0x20 LFN=READ_ONLY|HIDDEN|SYSTEM|VOLUME_ID (LFN means that this entry is a long file name entry)
    pub reserved: u8,
    pub creation_time_tenths: u8,
    pub creation_time: u16,
    pub creation_date: u16,
    pub last_access_date: u16,
    pub upper_first_cluster: u16,
    pub last_change_time: u16,
    pub last_change_date: u16,
    pub lower_first_cluster: u16,
    pub file_size: u32
}   // 32 byte

pub struct Directory {
    entries: Vec<DirectoryEntry>

    // ! Readonly (immutable slice reference)
    // entries: &'static[DirectoryEntry]

    // ! Unsafe if memory allocation is not handled, memory can be overwritten
    // entries: *const DirectoryEntry,
    // entries_count: u16
}

impl Directory {
    pub fn get_entry(&self, name: &str) -> Option<&DirectoryEntry> {
        for i in 0..self.entries.len() {
            // Get ith entry in the directory
            let entry: &DirectoryEntry = self.entries.get(i)?;

            // If the first byte is NULL, the previous entry was the last one
            if *entry.name.get(0)? == 0x00 { break; }

            // If the name is equal to the input, this is the entry
            if name.as_bytes().eq(&entry.name) { return Some(&entry); }
        }
        None
    }
}

/* ==== METHODS ============================================================= */
pub fn open_disk(path: &str) -> io::Result<File> {
    return File::open(path);
}

pub fn read_boot_sector(disk: &mut File) -> io::Result<BootSector> {
    return read_struct::<BootSector>(disk);
}

pub fn read_fat(disk: &mut File, boot_sector: &BootSector) -> io::Result<Fat> {

    // Calculate fat offset and size using boot sector data
    let fat_offset_start: u16 = boot_sector.get_fat_start();
    let fat_size: u16 = boot_sector.get_fat_size();

    // Seek the file to the correct location so that we can read the FAT
    disk.seek(SeekFrom::Start(fat_offset_start.into()))?;

    // Create a Vec already filled with disk data from seeked point
    let buffer: Vec<u8> = read_buffer(disk, fat_size as usize)?;

    // Create Fat struct with the retrieved allocated data pointer
    // Give Vec ownership to the struct so that it can write to the data
    return Ok( Fat { entries: buffer } );
}

pub fn read_root_directory(disk: &mut File, boot_sector: &BootSector) -> io::Result<Directory> {

    // Calculate fat offset and size using boot sector data
    let start: u16 = boot_sector.get_root_dir_start();
    let size: usize = boot_sector.get_root_dir_size();
    let count: usize = boot_sector.root_entries as usize;

    // Seek the file to the correct location so that we can read the FAT
    disk.seek(SeekFrom::Start(start.into()))?;

    // Create a Vec already filled with disk data from seeked point
    let temp_buffer: Vec<u8> = read_buffer(disk, size)?;

    // Transmute the Vec<u8> into Vec<MyStruct>
    let buffer: Vec<DirectoryEntry> = unsafe { Vec::from_raw_parts(temp_buffer.as_ptr() as *mut DirectoryEntry, count, count) };
    // let buffer: &[DirectoryEntry] = unsafe { from_raw_parts(buffer.as_ptr() as *const DirectoryEntry, count as usize) };

    // Prevent the Vec<u8> from deallocating new buffer's memory
    // This prevents .drop call, implemented in Vec with dealloc of pointed data
    mem::forget(temp_buffer);

    // Create Fat struct with the retrieved allocated data pointer
    // Give Vec ownership to the struct so that it can write to the data
    return Ok( Directory { entries: buffer } );
}

pub fn read_entry_content(disk: &mut File, entry: &DirectoryEntry, fat: &Fat, boot_sector: &BootSector) -> io::Result<Vec<u8>> {

    // Get the first cluster the data is stored in from the entry
    let mut current_cluster: u16 = entry.lower_first_cluster;

    // Get the size of the disk data that needs to be read
    let cluster_size: usize = boot_sector.get_cluster_size();

    // Setup data accumulator and temporary buffer
    let mut accumulator: Vec<u8> = vec![];
    let mut temp_buffer: Vec<u8>;
    loop {
        // Get offset of the given cluster in the disk
        let cluster_offset_start: usize = boot_sector.get_cluster_start(current_cluster);

        // Seek the file to the correct location so that we can read the file
        disk.seek(SeekFrom::Start(cluster_offset_start as u64))?;

        // Create a Vec already filled with disk data from seeked point
        temp_buffer = read_buffer(disk, cluster_size)?;

        // Concatenate previously retrieved data with the new data
        // Values are moved but ownership is given to accumulator again
        accumulator = [accumulator, temp_buffer].concat();

        // Check the FAT for the next cluster
        current_cluster = fat.get_entry(current_cluster as usize);

        // If the cluster number is higher than FF8, that was the last cluster
        if current_cluster >= 0x0FF8 { break; }
    }

    // Return the accumulated data
    Ok(accumulator)
}

/* ==== UTILS =============================================================== */
/** Read from file and fill bytebuffer of given size with the retrieved data. */
fn read_buffer(disk: &mut File, size: usize) -> io::Result<Vec<u8>> {
    // Buffer size known at run time: allocated in the heap
    // Create an uninitialized Vec, initialize bytes with resize to 0 fill it
    //* let mut buffer = Vec::with_capacity(buffer_size_runtime);
    //* buffer.resize(buffer_size_runtime, 0);

    // Create a Vec already filled with 0
    let mut buffer: Vec<u8> = vec![0; size];

    // Popolate the buffer with the first chunk of file content
    disk.read_exact(&mut buffer)?;

    // Print out buffer content
    //* println!("Buffer: {:02X?}", buffer);

    Ok(buffer)
}

/** Read from file and fill the given struct with the retrieved data. */
fn read_struct<T>(disk: &mut File) -> io::Result<T> {

    // Get buffer size dinamically - not known until runtime (we need the type)
    let type_size: usize = std::mem::size_of::<T>();

    // Create a Vec already filled with disk data from seeked point
    let buffer: Vec<u8> = read_buffer(disk, type_size)?;

    // Convert the buffer into the struct - Cast only isn't enought,
    // since we have to deal with byte alignment, and it's unsafe anyway
    // Take the buffer bytes as is, and convert pointer to our struct's pointer,
    // "assuming" that the raw data will fit correctly in the struct fields.
    // The Vec actually contains more data than the raw bytes of the file,
    // such as instance metadata: the "as_ptr" returns the pointer to raw data.
    let strct: T = unsafe { std::ptr::read_unaligned(buffer.as_ptr() as *const T) };


    /* COMPILE TIME OPTIMIZED IMPLEMENTATION
     * to be used when the size buffer is known - such as this case, but I wanted to try):
    
     * Get buffer size - fixed, known at compile time knowing the type
    let buffer_size = std::mem::size_of::<BootSector>();

     * Size known at compile time: create plain array allocated in the stack
    let mut buffer = [0; buffer_size];

     * Popolate the buffer with the first chunk of file content
    disk.read_exact(&mut buffer)?;

     * Convert the raw data of the array in struct data (avoid byte alignments)
    let strct: BootSector = unsafe { std::ptr::read_unaligned(&buffer as *const _ as *const BootSector) };
     */

    // Return the "filled" data structure
    Ok(strct)
}