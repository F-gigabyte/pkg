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

use std::env;

use clap::Parser;

/// Provided program arguments
#[derive(Debug)]
pub struct Args {
    /// Arguments provided through the command line
    pub cmd_args: CmdArgs,
    /// Arguments passed as environmental variables
    pub env_args: EnvArgs
}

impl Args {
    /// Parses the program arguments
    pub fn parse() -> Self {
        let env_args = EnvArgs::parse();
        let cmd_args = CmdArgs::parse();
        Self {
            env_args,
            cmd_args
        }
    }
}

/// Command line arguments
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct CmdArgs {
    /// Location of the config toml file
    pub config_file: String,
    /// Whether to build a release version
    #[arg(short, long, default_value_t = false)]
    pub release: bool,
    /// Whether to build a kernel release version (defaults to same as release)
    #[arg(long)]
    pub kernel_release: Option<bool>,
    /// Whether to build tests or the binaries themselves
    #[arg(short, long, default_value_t = false)]
    pub test: bool,
    /// Whether to build kernel tests or just the kernel binary (defaults to same as test)
    #[arg(long)]
    pub kernel_test: Option<bool>,
    /// The output file to put the final image in
    #[arg(short, long)]
    pub outfile: String
}

/// Environmental variable arguments
#[derive(Debug)]
pub struct EnvArgs {
    /// Which objcopy binary to use
    pub objcopy: String,
    /// Which ld binary to use
    pub ld: String
}

impl EnvArgs {
    /// Parses the environmental variables
    pub fn parse() -> Self {
        // Default to system objcopy
        let objcopy = env::var("OBJCOPY").unwrap_or("objcopy".to_string());
        // Default to system ld
        let ld = env::var("LD").unwrap_or("ld".to_string());
        Self { 
            objcopy, 
            ld 
        }
    }
}
