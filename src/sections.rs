/* 
 * Copyright 2026 Fraser Griffin
 *
 * This file is part of Pkg.
 *
 * Pkg is free software: you can redistribute it and/or modify it under 
 * the terms of the GNU General Public License as published by the Free Software Foundation, 
 * either version 3 of the License, or (at your option) any later version.
 *
 * Pkg is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; 
 * without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. 
 * See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU Lesser General Public License along with Pkg. 
 * If not, see <https://www.gnu.org/licenses/>. 
 * 
 */

use std::{collections::HashMap, fs, path::Path, process::Command};

use tempfile::TempDir;

use crate::{allocs::AllocInfo, cmds::check_cmd, errors::PkgError};

/// A section rename
pub struct SectionRename {
    /// Old section name
    pub old_name: String,
    /// New section name
    pub new_name: String,
}

/// A section
pub struct Section {
    /// Section name
    pub name: String,
    /// Section's flash address (may be same as RAM address)
    pub phys_addr: usize,
    /// Section's RAM address (may be same as flash address)
    pub virt_addr: usize,
}

/// Renames a file's sections and localises its symbols and puts the output file in `link_files`  
/// `objcopy` is the objcopy binary to use  
/// `root` is the root of the temporary directory  
/// `file` is the elf file the symbols should be read from  
/// `sections` is a list of sections to rename  
/// `symbols` is a list of symbols to localise  
/// `link_files` is a list of all the elf files that have had their sections renamed  
/// On success returns the output file while on error returns a `PkgError`
pub fn rename_file_sections(objcopy: &str, root: &TempDir, file: &str, sections: &Vec<SectionRename>, symbols: &Vec<String>, link_files: &mut Vec<String>) -> Result<String, PkgError> {
    let file_name = Path::new(&file).file_name().unwrap().to_string_lossy().to_string();
    let res_name = root.path().join(&file_name).to_string_lossy().to_string().to_string();
    link_files.push(res_name.clone());
    let mut cmd = Command::new(&objcopy);
    for sec in sections {
        cmd
            .arg("--rename-section")
            .arg(&format!("{}={}", sec.old_name, sec.new_name));
    }
    for symbol in symbols {
        cmd.arg(&format!("--localize-symbol={}", symbol));
    }
    cmd.arg(&file).arg(&res_name);
    check_cmd(cmd).map_err(|_| 
        PkgError::CmdError {
            cmd: objcopy.to_string() 
        }
    )?; 
    Ok(res_name)
}

/// Creates the final linker script file  
/// `root` is the root of the temporary directory  
/// `sections` is a list of sections in the linker script  
/// `alloc_info` is the allocation information  
/// `prog_table_file` is the program table object file  
/// `async_queues_file` is the asynchronous queues object file  
/// `sync_endpoints_file` is the synchronous endpoints object file  
/// `async_endpoints_file` is the asynchronous endpoints object file  
/// `args_file` is the kernel driver arguments object file
/// On success returns the path to the link file and on error returns a `PkgError`
pub fn create_link_file(
    root: &TempDir, 
    sections: &Vec<Section>, 
    alloc_info: &AllocInfo,
    prog_table_file: &str, 
    async_queues_file: Option<&str>,
    sync_endpoints_file: Option<&str>,
    async_endpoints_file: Option<&str>,
    args_file: &str
    ) -> Result<String, PkgError> {

    let mut link_data = String::new();
    let mut have_bss = false;
    link_data.push_str("SECTIONS {");
    for sec in sections {
        if sec.name == "kernel.bss" {
            have_bss = true;
        }
        let symbol_name = sec.name.replace(".", "_");
        link_data = format!(
            "{}
            \t{} 0x{:x} : AT(0x{:x}) {{
            \t\t*({});
                . = ALIGN(4);
            \t}} =0xffff
            \t__{}_phys_start = LOADADDR({});
            \t__{}_phys_end = LOADADDR({}) + SIZEOF({});
            \t__{}_virt_start = ADDR({});
            \t__{}_virt_end = ADDR({}) + SIZEOF({});", 
            link_data, 
            sec.name, 
            sec.virt_addr, 
            sec.phys_addr, 
            sec.name, 
            symbol_name, 
            sec.name,
            symbol_name,
            sec.name,
            sec.name,
            symbol_name, 
            sec.name,
            symbol_name,
            sec.name,
            sec.name
        );
    }
    if !have_bss {
        link_data = format!(
            "{}
            \t__kernel_bss_phys_start = 0;
            \t__kernel_bss_phys_end = 0;
            \t__kernel_bss_virt_start = 0;
            \t__kernel_bss_virt_end = 0;", 
            link_data
        );
    }
    link_data = format!("{}\n\tprogram_table 0x{:x} : AT(0x{:x}) {{\n\t\t__program_table = .;\n\t\t{}\n\t}}", link_data, alloc_info.prog_table_phys, alloc_info.prog_table_phys, prog_table_file);
    link_data = format!("{}\n\targs 0x{:x} : AT(0x{:x}) {{\n\t\t__args = .;\n\t\t{}\n\t}}", link_data, alloc_info.args, alloc_info.args, args_file);
    if let Some(sync_endpoints_file) = sync_endpoints_file {
        link_data = format!("{}\n\tsync_endpoints 0x{:x} : AT(0x{:x}) {{\n\t\t{}\n\t}}", link_data, alloc_info.sync_endpoints_phys, alloc_info.sync_endpoints_phys, sync_endpoints_file);
    }
    if let Some(async_endpoints_file) = async_endpoints_file {
        link_data = format!("{}\n\tasync_endpoints 0x{:x} : AT(0x{:x}) {{\n\t\t{}\n\t}}", link_data, alloc_info.async_endpoints_phys, alloc_info.async_endpoints_phys, async_endpoints_file);
    }
    if let Some(async_queues_file) = async_queues_file {
        link_data = format!(
            "{}
            \tasync_queues 0x{:x} : AT(0x{:x}) {{
            \t\t{}
            \t}}, 
            \t__async_queues_phys_start = LOADADDR(async_queues);
            \t__async_queues_phys_end = LOADADDR(async_queues) + SIZEOF(async_queues);
            \t__async_queues_virt_start = ADDR(async_queues);
            \t__async_queues_virt_end = ADDR(async_queues) + SIZEOF(async_queues);", 
            link_data, 
            alloc_info.async_queues_phys, 
            alloc_info.async_queues_virt, 
            async_queues_file
        );
    } else {
        link_data = format!(
            "{}
            \t__async_queues_phys_start = 0x{:x};
            \t__async_queues_phys_end = 0x{:x};
            \t__async_queues_virt_start = 0x{:x};
            \t__async_queues_virt_end = 0x{:x};", 
            link_data, 
            alloc_info.async_queues_phys, 
            alloc_info.async_queues_phys, 
            alloc_info.async_queues_virt, 
            alloc_info.async_queues_virt, 
        );
    }
    link_data = format!(
        "{}
        \t__sync_queues_virt_start = 0x{:x};
        \t__sync_queues_virt_end = 0x{:x} + 0x{:x};", 
        link_data, 
        alloc_info.sync_queues_virt, 
        alloc_info.sync_queues_virt, 
        alloc_info.sync_queues_len
    );
    link_data = format!(
        "{}
        \t__notifier_virt_start = 0x{:x};
        \t__notifier_virt_end = 0x{:x} + 0x{:x};", 
        link_data, 
        alloc_info.notifier_virt, 
        alloc_info.notifier_virt, 
        alloc_info.notifier_len
    );
    link_data = format!(
        "{}
        \t__procs_virt_start = 0x{:x};
        \t__procs_virt_end = 0x{:x} + 0x{:x};", 
        link_data, 
        alloc_info.proc_virt, 
        alloc_info.proc_virt, 
        alloc_info.proc_len
    );
    link_data = format!(
        "{}
        \t__scratch_data = 0x{:x};", 
        link_data, 
        alloc_info.codes
    );
    link_data = format!(
        "{}
        \t__kernel_stack = 0x{:x};
        }}
        ", 
        link_data, 
        alloc_info.kernel_stack
    );
    let link_file = root.path().join("link.ld");
    fs::write(&link_file, link_data).map_err(|_| 
        PkgError::WriteError {
            file: "link.ld".to_string()
        }
    )?;
    let link_file = link_file.to_string_lossy().to_string().to_string();
    Ok(link_file)
}

/// Prints the list of section renames  
/// `renames` is the list of section renames to print
pub fn print_renames(renames: &HashMap<String, (Vec<SectionRename>, Vec<String>)>) {
    for (file, (secs, _)) in renames.iter() {
        println!("{}", file);
        for sec in secs {
            println!("\t{} -> {}", sec.old_name, sec.new_name);
        }
    }
}
