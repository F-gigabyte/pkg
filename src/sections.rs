use std::{collections::HashMap, fs, path::Path, process::Command};

use crate::{allocs::AllocInfo, cmds::check_cmd, errors::PkgError};

pub struct SectionRename {
    pub old_name: String,
    pub new_name: String,
}

pub struct Section {
    pub name: String,
    pub phys_addr: usize,
    pub virt_addr: usize,
}

pub fn rename_file_sections(objcopy: &str, path: &Path, file: &str, sections: &Vec<SectionRename>, symbols: &Vec<String>, link_files: &mut Vec<String>) -> Result<String, PkgError> {
    let file_name = Path::new(&file).file_name().unwrap().to_string_lossy().to_string();
    let res_name = path.join(&file_name).to_string_lossy().to_string().to_string();
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

pub fn create_link_file(
    path: &Path, 
    sections: &Vec<Section>, 
    alloc_info: &AllocInfo,
    prog_table_file: &str, 
    async_queues_file: Option<&str>,
    sync_endpoints_file: Option<&str>,
    async_endpoints_file: Option<&str>
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
            \t}}
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
    let link_file = path.join("link.ld");
    fs::write(&link_file, link_data).map_err(|_| 
        PkgError::WriteError {
            file: "link.ld".to_string()
        }
    )?;
    let link_file = link_file.to_string_lossy().to_string().to_string();
    Ok(link_file)
}

pub fn print_renames(renames: &HashMap<String, (Vec<SectionRename>, Vec<String>)>) {
    for (file, (secs, _)) in renames.iter() {
        println!("{}", file);
        for sec in secs {
            println!("\t{} -> {}", sec.old_name, sec.new_name);
        }
    }
}
