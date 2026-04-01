use std::env;

use clap::Parser;

#[derive(Debug)]
pub struct Args {
    pub cmd_args: CmdArgs,
    pub env_args: EnvArgs
}

impl Args {
    pub fn parse() -> Self {
        let env_args = EnvArgs::parse();
        let cmd_args = CmdArgs::parse();
        Self {
            env_args,
            cmd_args
        }
    }
}

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

#[derive(Debug)]
pub struct EnvArgs {
    pub objcopy: String,
    pub ld: String
}

impl EnvArgs {
    pub fn parse() -> Self {
        let objcopy = env::var("OBJCOPY").unwrap_or("objcopy".to_string());
        let ld = env::var("LD").unwrap_or("ld".to_string());
        Self { 
            objcopy, 
            ld 
        }
    }
}
