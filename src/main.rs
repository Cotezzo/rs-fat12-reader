use std::{env::{self, Args}, fs::File};
use rs_disk_reader::{BootSector, open_disk, read_boot_sector, Fat, read_fat, Directory, read_root_directory, DirectoryEntry, read_entry_content};

/* ==== MAIN ================================================================ */
fn main() {
    let mut args: Args = env::args();
    
    args.next();
    let image_path: String = args.next().expect( "Didn't get an image path");
    let file_name: String = args.next().expect("Didn't get a file to be read");

    let mut disk: File = open_disk(&image_path).expect("Could not open image");
    let boot_sector: BootSector = read_boot_sector(&mut disk).expect("Could not read image");
    let fat: Fat = read_fat(&mut disk, &boot_sector).expect("Could not read FAT from image");
    let root_directory: Directory = read_root_directory(&mut disk, &boot_sector).expect("Could not read Root Dir from image");
    let kernel_entry: &DirectoryEntry = root_directory.get_entry(&file_name).expect("Could not find file in image");
    let kernel_binary: Vec<u8> = read_entry_content(&mut disk, &kernel_entry, &fat, &boot_sector).expect("Could not read file from image");
    
    println!("File content: {:02X?}", kernel_binary);
}