# rs-fat12-reader
## Introducton
Small and unsafe bare bones Rust implementation of a FAT12 "driver" that reads files from the root entries of the data clusters. Made as a "training" for both my Rust knowledge and the actual assembly implementation for my [Porcheria-OS](https://github.com/Cotezzo/porcheria-os) project.

I'll probably want to refine this implementation in the future.

## Usage
### Installation
This is a command line program, the first parameter is the FAT12 disk image you want to read, the second one is the file you want to read. Outputs the file content as a byte array (hex). Panics if something goes wrong in the process. I've included a test image containing "kernel.bin" and "bigfile.txt" to try out the program.

Build and run with cargo: 
- `cargo run -- test_floppy.img "KERNEL  BIN"`